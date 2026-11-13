//! 原生镜像最低 lowering 的结构化验证与真实运行证据。
//!
//! native 不是平台：用例显式给出规范目标四元组 `x86_64-pc-windows-msvc`，验证
//! Win64 参数传参、同片段调用、冻结 x86-64 算子、条件分支与 `Return` 生成的
//! `PE32+`；在 Windows 上真实运行生成的 `.exe`，用进程退出码作为运行时证据。
//! 未闭合的目标四元组、未冻结绑定或超出边界的形状必须结构化失败，不得生成
//! 占位镜像或借用其他平台的编码与容器。

use std::{collections::BTreeMap, process::Command};

use nyar::{
    ExternalImportLink, Identifier, QualifiedName,
    abstractions::{BinaryArch, CanonicalTarget},
};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    lowering::backends::native::x86_64::X86_64OperatorCode,
    nyar_backend_native::{NativeExecutableKind, classify_native_image},
    target_operator::{TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe},
    testing::lower_fragment_to_native_image,
};
use nyar_types::{
    Block, BlockRef, Constant, ExecutableFunction, ImportIndex, Instruction, InstructionKind, ItemInstanceId, NyarType, Operand, Terminator,
    ValueRef,
};

const LONG: NyarType = NyarType::Integer64 { signed: true };
const INTEGER: NyarType = NyarType::Integer32 { signed: true };

/// 冻结 native operator 的 import 实例身份（远离函数实例区间）。
const OPERATOR_INSTANCE_BASE: u32 = 100;
/// 失败闭合用例使用的宿主导入实例身份。
const HOST_IMPORT_INSTANCE: u32 = 200;
/// 失败闭合用例使用的未知调用身份。
const UNKNOWN_CALLEE: u32 = 150;

/// 本切片闭合的规范目标四元组。
fn windows_x64() -> CanonicalTarget {
    CanonicalTarget::parse("x86_64-pc-windows-msvc").expect("canonical target")
}

fn lower_windows_x64(submission: &FragmentSubmission) -> nyar_emitter::Result<nyar_emitter::lowering::backends::native::NativeLoweredFragment> {
    lower_fragment_to_native_image(submission, windows_x64())
}

fn function(symbol: &str, return_type: NyarType, parameters: Vec<NyarType>) -> ExecutableFunction {
    let value_types = parameters.iter().enumerate().map(|(index, ty)| (ValueRef(index as u32), ty.clone())).collect();
    let block_parameters = (0..parameters.len() as u32).map(ValueRef).collect();
    ExecutableFunction {
        return_layout: None,
        value_layouts: BTreeMap::new(),
        symbol: symbol.to_string(),
        return_type,
        param_types: parameters,
        value_types,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: block_parameters,
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        }],
        diagnostics: Vec::new(),
    }
}

fn instr(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    let mut instruction = Instruction::from_kind(kind);
    instruction.results = results;
    instruction
}

fn call(instance: ItemInstanceId, arguments: Vec<Operand>, results: Vec<ValueRef>) -> Instruction {
    instr(InstructionKind::Call { callee: Operand::Item(instance), arguments }, results)
}

fn integer(value: i64) -> Operand {
    Operand::Constant(Constant::Int(value))
}

fn operator_plan(code: X86_64OperatorCode) -> TargetOperatorPlan {
    let result_type = if code.is_comparison() { TargetOperandType::I32 } else { TargetOperandType::I64 };
    TargetOperatorPlan {
        arity: code.arity(),
        operand_type: TargetOperandType::I64,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::X86_64(code),
    }
}

fn operator_import(code: X86_64OperatorCode, return_type: NyarType) -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(0).expect("import index"),
        link: ExternalImportLink::host(Some(Identifier::new("native-operator")), vec![format!("{code:?}")]),
        parameter_types: vec![LONG; code.arity()],
        return_type,
        target_operator: Some(TargetOperatorBinding::Frozen(operator_plan(code))),
    }
}

fn operator_instance(offset: u32) -> ItemInstanceId {
    ItemInstanceId::from_index(OPERATOR_INSTANCE_BASE + offset).expect("operator instance")
}

/// 从已构造的计划中按符号名解析函数实例身份。
fn instance_of(plan: &TargetExecutionPlan, symbol: &str) -> ItemInstanceId {
    plan.testing_functions()
        .iter()
        .find(|(_, function)| function.symbol == symbol)
        .map(|(instance, _)| *instance)
        .unwrap_or_else(|| panic!("fixture function `{symbol}` 不在计划中"))
}

fn with_function_mut(plan: &mut TargetExecutionPlan, symbol: &str, mutate: impl FnOnce(&mut ExecutableFunction)) {
    let function = plan
        .testing_functions_mut()
        .iter_mut()
        .find(|(_, function)| function.symbol == symbol)
        .map(|(_, function)| function)
        .unwrap_or_else(|| panic!("fixture function `{symbol}` 不在计划中"));
    mutate(function);
}

fn expect_pe(executable: &[u8]) {
    let facts = classify_native_image(executable).expect("native 产物必须是可解析镜像");
    assert_eq!(facts.kind, NativeExecutableKind::Pe, "必须是 PE32+ 而不是 ELF");
    assert_eq!(facts.arch, BinaryArch::X64, "必须是 AMD64 镜像");
    assert!(!facts.shared, "必须是可执行镜像而不是共享库");
}

/// 在 Windows 上真实运行生成的 `.exe`，返回进程退出码。
#[cfg(windows)]
fn run_exit_code(name: &str, executable: &[u8]) -> i32 {
    let directory = std::env::temp_dir().join(format!("nyar-native-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("native temp dir");
    let path = directory.join(format!("{name}.exe"));
    std::fs::write(&path, executable).expect("write native exe");
    let status = Command::new(&path).status().expect("run native exe");
    let code = status.code().expect("native exe exit code");
    let _ = std::fs::remove_dir_all(&directory);
    code
}

/// `add(a, b) = a + b`（冻结 `I64Add`），`main() = add(40, 2)`。
///
/// 覆盖：Win64 前两个形参寄存器、同片段 `call rel32`、冻结算子、`Return`。
#[test]
fn parameters_calls_and_frozen_operator_run_natively() {
    let add = function("add", LONG, vec![LONG, LONG]);
    let mut main = function("main", LONG, Vec::new());
    main.value_types.insert(ValueRef(0), LONG);
    main.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };

    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let add_name = QualifiedName::new(vec![Identifier::new("add")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), main), (add_name, add)]));
    plan.testing_module_name("native_lowering").testing_entry_operation(main_name);

    let add_instance = instance_of(&plan, "add");
    with_function_mut(&mut plan, "add", |function| {
        function.value_types.insert(ValueRef(2), LONG);
        function.blocks[0].instructions.push(call(
            operator_instance(0),
            vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))],
            vec![ValueRef(2)],
        ));
        function.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };
    });
    with_function_mut(&mut plan, "main", |function| {
        function.blocks[0].instructions.push(call(add_instance, vec![integer(40), integer(2)], vec![ValueRef(0)]));
    });
    plan.testing_register_import(operator_instance(0), operator_import(X86_64OperatorCode::I64Add, LONG));

    let lowered = lower_windows_x64(&FragmentSubmission::from_backend_plan(plan)).expect("native 纵切必须生成 PE");
    expect_pe(&lowered.executable);
    assert!(lowered.text_len > 0, "`.text` 必须包含真实机器码");

    #[cfg(windows)]
    assert_eq!(run_exit_code("native-add", &lowered.executable), 42, "add(40, 2) 的进程退出码");
}

/// `choose(a, b) = if a < b { a + b } else { a - b }`，`main() = choose(3, 4)`。
///
/// 覆盖：冻结比较算子、条件分支、两个算术算子与块内 `Return`。
#[test]
fn branch_through_frozen_compare_runs_natively() {
    let mut choose = function("choose", LONG, vec![LONG, LONG]);
    choose.value_types.insert(ValueRef(2), INTEGER);
    choose.value_types.insert(ValueRef(3), LONG);
    choose.value_types.insert(ValueRef(4), LONG);
    choose.blocks[0].instructions.push(call(
        operator_instance(1),
        vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))],
        vec![ValueRef(2)],
    ));
    choose.blocks[0].terminator =
        Terminator::Branch { condition: Operand::Value(ValueRef(2)), then_target: BlockRef(1), else_target: BlockRef(2) };
    choose.blocks.push(Block {
        id: BlockRef(1),
        label: "then".to_string(),
        parameters: Vec::new(),
        instructions: vec![call(operator_instance(2), vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))], vec![ValueRef(3)])],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(3))) },
    });
    choose.blocks.push(Block {
        id: BlockRef(2),
        label: "else".to_string(),
        parameters: Vec::new(),
        instructions: vec![call(operator_instance(3), vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))], vec![ValueRef(4)])],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(4))) },
    });

    let mut main = function("main", LONG, Vec::new());
    main.value_types.insert(ValueRef(0), LONG);
    main.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };

    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let choose_name = QualifiedName::new(vec![Identifier::new("choose")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), main), (choose_name, choose)]));
    plan.testing_module_name("native_lowering").testing_entry_operation(main_name);

    let choose_instance = instance_of(&plan, "choose");
    with_function_mut(&mut plan, "main", |function| {
        function.blocks[0].instructions.push(call(choose_instance, vec![integer(3), integer(4)], vec![ValueRef(0)]));
    });
    plan.testing_register_import(operator_instance(1), operator_import(X86_64OperatorCode::I64CompareLessThan, INTEGER));
    plan.testing_register_import(operator_instance(2), operator_import(X86_64OperatorCode::I64Add, LONG));
    plan.testing_register_import(operator_instance(3), operator_import(X86_64OperatorCode::I64Subtract, LONG));

    let lowered = lower_windows_x64(&FragmentSubmission::from_backend_plan(plan)).expect("分支纵切必须生成 PE");
    expect_pe(&lowered.executable);

    #[cfg(windows)]
    assert_eq!(run_exit_code("native-branch", &lowered.executable), 7, "choose(3, 4) = 3 + 4 的进程退出码");
}

#[test]
fn unit_return_exits_zero() {
    let main = function("main", NyarType::Unit, Vec::new());
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), main)]));
    plan.testing_module_name("native_lowering").testing_entry_operation(main_name);

    let lowered = lower_windows_x64(&FragmentSubmission::from_backend_plan(plan)).expect("unit 入口必须生成 PE");
    expect_pe(&lowered.executable);

    #[cfg(windows)]
    assert_eq!(run_exit_code("native-unit", &lowered.executable), 0, "unit 入口退出码");
}

/// 宿主导入没有冻结 native 编码：必须结构化失败，而不是生成不调用任何东西的镜像。
#[test]
fn host_import_without_binding_is_rejected() {
    let mut main = function("main", LONG, Vec::new());
    main.value_types.insert(ValueRef(0), LONG);
    main.blocks[0].instructions.push(call(
        ItemInstanceId::from_index(HOST_IMPORT_INSTANCE).expect("host import instance"),
        Vec::new(),
        vec![ValueRef(0)],
    ));
    main.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };

    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), main)]));
    plan.testing_module_name("native_lowering").testing_entry_operation(main_name);
    plan.testing_register_import(
        ItemInstanceId::from_index(HOST_IMPORT_INSTANCE).expect("host import instance"),
        BackendImport {
            index: ImportIndex::from_index(0).expect("import index"),
            link: ExternalImportLink::host(Some(Identifier::new("host")), vec!["println".to_string()]),
            parameter_types: Vec::new(),
            return_type: LONG,
            target_operator: None,
        },
    );

    let error = lower_windows_x64(&FragmentSubmission::from_backend_plan(plan)).expect_err("宿主导入必须失败闭合");
    assert!(error.contains("宿主导入"), "错误必须点名宿主导入边界: {error}");
}

#[test]
fn unfrozen_operator_binding_is_rejected() {
    let submission = single_operator_submission(TargetOperatorBinding::Unfrozen, LONG);
    let error = lower_windows_x64(&submission).expect_err("未冻结绑定必须失败");
    assert!(error.contains("未冻结"), "错误必须点名未冻结绑定: {error}");
}

#[test]
fn non_native_operator_encoding_is_rejected() {
    let submission = single_operator_submission(
        TargetOperatorBinding::Frozen(TargetOperatorPlan {
            arity: 2,
            operand_type: TargetOperandType::I64,
            result_type: TargetOperandType::I64,
            recipe: TargetOperatorRecipe::Operands,
            encoding: TargetOperatorEncoding::Wasm(0x6A),
        }),
        LONG,
    );
    let error = lower_windows_x64(&submission).expect_err("非 x86-64 编码必须失败");
    assert!(error.contains("非 x86-64 编码"), "错误必须点名非 x86-64 编码: {error}");
}

/// 构造 `main() = operator(10, 3)`：调用指定绑定的一次算子。
fn single_operator_submission(binding: TargetOperatorBinding, return_type: NyarType) -> FragmentSubmission {
    let mut main = function("main", return_type.clone(), Vec::new());
    main.value_types.insert(ValueRef(0), return_type.clone());
    main.blocks[0].instructions.push(call(operator_instance(0), vec![integer(10), integer(3)], vec![ValueRef(0)]));
    main.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };

    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), main)]));
    plan.testing_module_name("native_lowering").testing_entry_operation(main_name);
    plan.testing_register_import(
        operator_instance(0),
        BackendImport {
            index: ImportIndex::from_index(0).expect("import index"),
            link: ExternalImportLink::host(Some(Identifier::new("native-operator")), vec!["probe".to_string()]),
            parameter_types: vec![LONG, LONG],
            return_type,
            target_operator: Some(binding),
        },
    );
    FragmentSubmission::from_backend_plan(plan)
}

#[test]
fn entry_with_parameters_is_rejected() {
    let mut main = function("main", LONG, vec![LONG]);
    main.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), main)]));
    plan.testing_module_name("native_lowering").testing_entry_operation(main_name);

    let error = lower_windows_x64(&FragmentSubmission::from_backend_plan(plan)).expect_err("带参入口必须失败");
    assert!(error.contains("必须无参数"), "错误必须说明 PE 入口不能带形参: {error}");
}

#[test]
fn call_to_unknown_instance_is_rejected() {
    let mut main = function("main", LONG, Vec::new());
    main.value_types.insert(ValueRef(0), LONG);
    main.blocks[0].instructions.push(call(
        ItemInstanceId::from_index(UNKNOWN_CALLEE).expect("unknown instance"),
        Vec::new(),
        vec![ValueRef(0)],
    ));
    main.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };

    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), main)]));
    plan.testing_module_name("native_lowering").testing_entry_operation(main_name);

    let error = lower_windows_x64(&FragmentSubmission::from_backend_plan(plan)).expect_err("未知调用目标必须失败");
    assert!(error.contains("不在可达函数集"), "错误必须点名不可达调用目标: {error}");
}

/// 未闭合的规范目标四元组必须失败闭合，不得把 Windows `PE` 编码借给其他平台。
#[test]
fn unclosed_canonical_targets_are_rejected() {
    let main = function("main", NyarType::Unit, Vec::new());
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), main)]));
    plan.testing_module_name("native_lowering").testing_entry_operation(main_name);
    let submission = FragmentSubmission::from_backend_plan(plan);

    let linux = CanonicalTarget::parse("x86_64-unknown-linux-gnu").expect("linux quad");
    let error = lower_fragment_to_native_image(&submission, linux).expect_err("ELF 容器未实现必须失败");
    assert!(error.contains("容器实现"), "错误必须点名缺失的容器实现: {error}");

    let aarch64 = CanonicalTarget::parse("aarch64-unknown-linux-gnu").expect("aarch64 quad");
    let error = lower_fragment_to_native_image(&submission, aarch64).expect_err("aarch64 未实现必须失败");
    assert!(error.contains("尚未实现架构 `aarch64`"), "错误必须点名未实现的 ISA: {error}");
}
