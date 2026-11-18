//! 正式编译 lane 的宿主扩展合同验收。
//!
//! 覆盖：冻结导出面 → 类型化宿主边界（`i64`/`bool`）、Node-API / CPython 承载
//! 装配、共享库容器与宿主装载入口符号；未闭合的组合（进程入口、空导出面、
//! 窄整数实参、字符串返回、缺失 Windows CPython 导入库、非法模块名）必须在
//! 写出前结构化失败，不产出改标或补造的扩展交付。

use std::collections::BTreeMap;

use nyar::{
    BackendInputKind, ExternalImportLink, HostProjectionBoundary, Identifier, PartitionBackendRequirement, ReferenceManagement, TargetLane,
    abstractions::{BinaryArch, BinaryFlavor, BinaryTarget, TargetFamily},
    backends::CompilationOptions,
};
use nyar_emitter::{
    FragmentSubmission, LoweredBackendInput, TargetExecutionPlan,
    contracts::{Constant, InstructionKind, Operand, Terminator, ValueRef},
    lowering::backends::machine::x86_64::X86_64OperatorCode,
    nyar_backend_machine::{MachineImageBackendInput, HostModule, X64Platform, X64ProductTarget, classify_machine_image},
    target_operator::TargetOperandType,
    testing::{compile_lowered_backend_input, lower_fragment_to_machine_extension},
};
use nyar_emitter::contracts::{Block, BlockRef, ExecutableFunction, Instruction};
use nyar_types::{ImportIndex, ItemInstanceId, SemanticType};

const LONG: SemanticType = SemanticType::Integer64 { signed: true };
const INTEGER: SemanticType = SemanticType::Integer32 { signed: true };
const OPERATOR_INSTANCE_BASE: u32 = 100;

fn function(symbol: &str, return_type: SemanticType, parameters: Vec<SemanticType>) -> ExecutableFunction {
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
        #[allow(deprecated)]
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: block_parameters,
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        }],
    }
}

fn instr(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    let mut instruction = Instruction::from_kind(kind);
    instruction.results = results;
    instruction
}

fn call(operator: ItemInstanceId, arguments: Vec<Operand>, results: Vec<ValueRef>) -> Instruction {
    instr(InstructionKind::Call { callee: Operand::Item(operator), arguments }, results)
}

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

fn operator_import(code: X86_64OperatorCode, return_type: SemanticType) -> nyar_emitter::BackendImport {
    let mnemonic = match code {
        X86_64OperatorCode::I64Add => "long.add",
        _ => "long.add",
    };
    nyar_emitter::BackendImport {
        index: ImportIndex::from_index(0).expect("import index"),
        link: ExternalImportLink::host(Some(Identifier::new("x86_64-operator")), vec![mnemonic.to_string()]),
        parameter_types: vec![match code.operand_type() {
            TargetOperandType::I64 => LONG,
            _ => INTEGER,
        }; code.arity()],
        return_type,
    }
}

fn operator_instance(offset: u32) -> ItemInstanceId {
    ItemInstanceId::from_index(OPERATOR_INSTANCE_BASE + offset).expect("operator instance")
}

/// `add(a, b) = a + b` 作为唯一导出，无进程入口（宿主扩展交付形状）。
fn exported_add_plan() -> TargetExecutionPlan {
    let mut add = function("add", LONG, vec![LONG, LONG]);
    add.value_types.insert(ValueRef(2), LONG);
    add.blocks[0].instructions.push(call(
        operator_instance(0),
        vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))],
        vec![ValueRef(2)],
    ));
    add.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let add_name = nyar::QualifiedName::new(vec![Identifier::new("add")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(add_name, add)]));
    plan.testing_module_name("native_extension");
    plan.testing_register_import(operator_instance(0), operator_import(X86_64OperatorCode::I64Add, LONG));
    let add_instance = instance_of(&plan, "add");
    plan.testing_export_names_mut().insert(add_instance, "add".to_string());
    plan.testing_exported_operations_mut().push(add_instance);
    plan
}

fn node(platform: X64Platform) -> X64ProductTarget {
    X64ProductTarget::extension(platform, HostModule::Node)
}

fn python(platform: X64Platform, library: Option<&str>) -> X64ProductTarget {
    python_with_suffix(platform, library, None)
}

fn python_with_suffix(platform: X64Platform, library: Option<&str>, suffix: Option<&str>) -> X64ProductTarget {
    X64ProductTarget::Extension {
        platform,
        module: HostModule::Python,
        cpython: Some(nyar::target_resolution::CpythonHostConfig {
            module_suffix: suffix.map(str::to_string),
            import_library: library.map(str::to_string),
        }),
    }
}

/// 探测/配置冻结的 CPython 扩展后缀直接进入产物命名，不按 OS 重新猜。
#[test]
fn probed_module_suffix_drives_artifact_extension() {
    let submission = FragmentSubmission::from_backend_plan(exported_add_plan());
    let lowered = lower_fragment_to_machine_extension(
        &submission,
        &python_with_suffix(X64Platform::windows_x64(), Some("python312.dll"), Some(".cp312-win_amd64.pyd")),
        "demo",
    )
    .expect("带探测后缀的 CPython 扩展必须产出");
    assert_eq!(lowered.artifact_extension, ".cp312-win_amd64.pyd", "产物扩展名必须来自冻结的宿主 ABI 配置");
}

/// Node-API Windows：PE DLL、注册入口符号与冻结导出面。
#[test]
fn node_extension_on_windows_writes_pe_dll_with_registration_entry() {
    let submission = FragmentSubmission::from_backend_plan(exported_add_plan());
    let lowered = lower_fragment_to_machine_extension(&submission, &node(X64Platform::windows_x64()), "demo")
        .expect("Windows Node 扩展必须产出共享库");
    assert_eq!(lowered.entry_symbol, "napi_register_module_v1");
    assert_eq!(lowered.host_exports, vec!["add".to_string()]);
    assert_eq!(lowered.artifact_extension, ".node");
    let facts = classify_machine_image(&lowered.executable).expect("镜像必须可解析");
    assert!(facts.shared, "宿主扩展必须是共享库镜像: {facts:?}");
    assert_eq!(facts.kind, nyar_emitter::nyar_backend_machine::MachineImageKind::Pe);
}

/// Node-API Linux：同一 ISA 与承载服务，容器换成 `ET_DYN`。
#[test]
fn node_extension_on_linux_writes_shared_elf() {
    let submission = FragmentSubmission::from_backend_plan(exported_add_plan());
    let lowered = lower_fragment_to_machine_extension(&submission, &node(X64Platform::linux_x64()), "demo")
        .expect("Linux Node 扩展必须产出 ET_DYN");
    assert_eq!(lowered.entry_symbol, "napi_register_module_v1");
    let facts = classify_machine_image(&lowered.executable).expect("镜像必须可解析");
    assert!(facts.shared, "宿主扩展必须是共享库镜像: {facts:?}");
}

/// CPython Linux：`PyInit_<module>` 入口与 `.so` 扩展名。
#[test]
fn python_extension_on_linux_writes_pyinit_entry() {
    let submission = FragmentSubmission::from_backend_plan(exported_add_plan());
    let lowered = lower_fragment_to_machine_extension(&submission, &python(X64Platform::linux_x64(), Some("libpython3.12.so")), "demo")
        .expect("Linux CPython 扩展必须产出 .so");
    assert_eq!(lowered.entry_symbol, "PyInit_demo");
    assert_eq!(lowered.artifact_extension, ".so");
    let facts = classify_machine_image(&lowered.executable).expect("镜像必须可解析");
    assert!(facts.shared);
}

/// Windows CPython 的导入 DLL 名必须由构建环境提供，缺失时在写出前拒绝。
#[test]
fn python_extension_on_windows_without_import_library_is_rejected() {
    let submission = FragmentSubmission::from_backend_plan(exported_add_plan());
    let error = lower_fragment_to_machine_extension(&submission, &python(X64Platform::windows_x64(), None), "demo")
        .expect_err("Windows CPython 缺导入库必须失败");
    assert!(error.contains("python312.dll") || error.contains("导入 DLL"), "错误必须点名导入库事实: {error}");
    assert!(error.contains("不能由编译器猜"), "{error}");
}

/// CPython 模块名必须能进 `PyInit_<module>`：非法名在写出前拒绝。
#[test]
fn python_extension_rejects_module_name_that_is_not_a_c_identifier() {
    let submission = FragmentSubmission::from_backend_plan(exported_add_plan());
    let error = lower_fragment_to_machine_extension(&submission, &python(X64Platform::linux_x64(), Some("libpython3.12.so")), "demo-app")
        .expect_err("非法 C 标识符必须失败");
    assert!(error.contains("C 标识符"), "{error}");
}

/// 宿主扩展不接受进程入口：入口与导出面是独立事实。
#[test]
fn host_extension_rejects_frozen_entry_operation() {
    let mut plan = exported_add_plan();
    let add_name = nyar::QualifiedName::new(vec![Identifier::new("add")]);
    plan.testing_entry_operation(add_name);
    let submission = FragmentSubmission::from_backend_plan(plan);
    let error = lower_fragment_to_machine_extension(&submission, &node(X64Platform::windows_x64()), "demo")
        .expect_err("带进程入口的计划不得作为宿主扩展交付");
    assert!(error.contains("不接受进程入口"), "{error}");
}

/// 空导出面没有可装载的宿主 API：写出前拒绝。
#[test]
fn host_extension_rejects_empty_export_surface() {
    let add = function("helper", LONG, vec![LONG]);
    let helper_name = nyar::QualifiedName::new(vec![Identifier::new("helper")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(helper_name, add)]));
    plan.testing_module_name("native_extension");
    let submission = FragmentSubmission::from_backend_plan(plan);
    let error = lower_fragment_to_machine_extension(&submission, &node(X64Platform::windows_x64()), "demo")
        .expect_err("无导出面必须失败");
    assert!(error.contains("没有可达函数") || error.contains("导出面为空"), "{error}");
}

/// 宿主边界只闭合完整 `i64` 实参：窄整数没有无损载体，拒绝而不是静默扩展。
#[test]
fn host_extension_rejects_narrow_integer_parameters() {
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(
        nyar::QualifiedName::new(vec![Identifier::new("narrow")]),
        function("narrow", LONG, vec![INTEGER]),
    )]));
    plan.testing_module_name("native_extension");
    with_function_mut(&mut plan, "narrow", |function| {
        function.blocks[0].terminator = Terminator::Return { value: Some(Operand::Constant(Constant::Int(7))) };
    });
    let instance = instance_of(&plan, "narrow");
    plan.testing_export_names_mut().insert(instance, "narrow".to_string());
    plan.testing_exported_operations_mut().push(instance);
    let submission = FragmentSubmission::from_backend_plan(plan);
    let error = lower_fragment_to_machine_extension(&submission, &node(X64Platform::windows_x64()), "demo")
        .expect_err("i32 实参必须拒绝跨越宿主边界");
    assert!(error.contains("无损宿主载体"), "{error}");
}

/// 形参数量超出宿主 ABI 窗口：写出前拒绝，不静默丢参。
#[test]
fn host_extension_rejects_parameters_beyond_the_abi_window() {
    let params = vec![LONG; 5];
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(
        nyar::QualifiedName::new(vec![Identifier::new("wide")]),
        function("wide", LONG, params),
    )]));
    plan.testing_module_name("native_extension");
    with_function_mut(&mut plan, "wide", |function| {
        function.blocks[0].terminator = Terminator::Return { value: Some(Operand::Constant(Constant::Int(1))) };
    });
    let instance = instance_of(&plan, "wide");
    plan.testing_export_names_mut().insert(instance, "wide".to_string());
    plan.testing_exported_operations_mut().push(instance);
    let submission = FragmentSubmission::from_backend_plan(plan);
    // Win64 只有 4 个整数实参寄存器；第 5 个形参超出 Node-API 回调窗口。
    let error = lower_fragment_to_machine_extension(&submission, &node(X64Platform::windows_x64()), "demo")
        .expect_err("超出寄存器传参上限的导出必须拒绝");
    assert!(error.contains("上限") || error.contains("形参"), "{error}");
}

/// 无符号 `u64` 返回没有有符号 `i64` 无损载体：与实参合同对齐，写出前拒绝。
#[test]
fn host_extension_rejects_unsigned_return_without_bigint_carrier() {
    let unsigned = SemanticType::Integer64 { signed: false };
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(
        nyar::QualifiedName::new(vec![Identifier::new("bits")]),
        function("bits", unsigned, Vec::new()),
    )]));
    plan.testing_module_name("native_extension");
    with_function_mut(&mut plan, "bits", |function| {
        function.blocks[0].terminator = Terminator::Return { value: Some(Operand::Constant(Constant::Int(-1))) };
    });
    let instance = instance_of(&plan, "bits");
    plan.testing_export_names_mut().insert(instance, "bits".to_string());
    plan.testing_exported_operations_mut().push(instance);
    let submission = FragmentSubmission::from_backend_plan(plan);
    let error = lower_fragment_to_machine_extension(&submission, &node(X64Platform::windows_x64()), "demo")
        .expect_err("u64 返回在 BigInt 载体前必须拒绝");
    assert!(error.contains("BigInt") || error.contains("有符号"), "{error}");
}

/// 正式 lane 尚未降低字符串常量：字符串返回在写出前拒绝，不伪造空字符串。
#[test]
fn host_extension_rejects_string_return() {
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(
        nyar::QualifiedName::new(vec![Identifier::new("text")]),
        function("text", SemanticType::Utf8, Vec::new()),
    )]));
    plan.testing_module_name("native_extension");
    let instance = instance_of(&plan, "text");
    plan.testing_export_names_mut().insert(instance, "text".to_string());
    plan.testing_exported_operations_mut().push(instance);
    let submission = FragmentSubmission::from_backend_plan(plan);
    let error = lower_fragment_to_machine_extension(&submission, &node(X64Platform::windows_x64()), "demo")
        .expect_err("字符串返回尚未闭合宿主载体，必须拒绝");
    assert!(error.contains("返回载体"), "{error}");
}

/// 驱动层：宿主扩展输入缺产物扩展名或导出面时，写出前拒绝。
#[test]
fn driver_rejects_host_extension_input_without_identity_facts() {
    let submission = FragmentSubmission::from_backend_plan(exported_add_plan());
    let lowered = lower_fragment_to_machine_extension(&submission, &node(X64Platform::windows_x64()), "demo").expect("扩展降低");
    let output_dir = tempfile::tempdir().expect("temp dir");
    let requirement = PartitionBackendRequirement {
        backend_name: "machine-code".to_string(),
        interpreter: Identifier::new("machine.image"),
        fragment: Identifier::new("functions"),
        lane: TargetLane::MachineCode,
        input_kind: BackendInputKind::PeImage,
        target: BinaryTarget::new(TargetFamily::MachineCode, BinaryArch::X64, BinaryFlavor::MachineCode),
        host_boundary: HostProjectionBoundary::MachineCode,
        reference_management: ReferenceManagement::PerceusRc,
    };
    let options = CompilationOptions {
        target: requirement.target.clone(),
        artifact_name: "demo".to_string(),
        emit_debug_symbols: false,
        optimize: false,
    };

    // 缺导出面。
    let input = LoweredBackendInput::machine(MachineImageBackendInput {
        executable: lowered.executable.clone(),
        output_dir: output_dir.path().to_path_buf(),
        entry_symbol: lowered.entry_symbol.clone(),
        artifact_extension: Some(".node".to_string()),
        host_exports: Vec::new(),
    });
    let error = compile_lowered_backend_input("demo", requirement.clone(), input, false, &options, nyar::ArtifactPurpose::HostExtension)
        .expect_err("无导出面的扩展交付必须拒绝");
    assert!(error.to_string().contains("导出面"), "{error}");

    // 缺产物扩展名。
    let input = LoweredBackendInput::machine(MachineImageBackendInput {
        executable: lowered.executable.clone(),
        output_dir: output_dir.path().to_path_buf(),
        entry_symbol: lowered.entry_symbol.clone(),
        artifact_extension: None,
        host_exports: lowered.host_exports.clone(),
    });
    let error = compile_lowered_backend_input("demo", requirement.clone(), input, false, &options, nyar::ArtifactPurpose::HostExtension)
        .expect_err("不具名的扩展交付必须拒绝");
    assert!(error.to_string().contains("扩展名"), "{error}");

    // 完整事实：写出成功，交付宿主导出索引而不是 CLI 运行合同。
    let input = LoweredBackendInput::machine(MachineImageBackendInput {
        executable: lowered.executable.clone(),
        output_dir: output_dir.path().to_path_buf(),
        entry_symbol: lowered.entry_symbol.clone(),
        artifact_extension: Some(".node".to_string()),
        host_exports: lowered.host_exports.clone(),
    });
    let report = compile_lowered_backend_input("demo", requirement, input, false, &options, nyar::ArtifactPurpose::HostExtension)
        .expect("完整宿主扩展交付必须写出");
    assert!(report.run_contracts.is_empty(), "宿主扩展没有 CLI 运行合同");
    assert_eq!(report.entry_symbol.as_deref(), Some("napi_register_module_v1"));
    assert_eq!(report.library_exports.len(), 1);
    assert_eq!(report.library_exports[0].logical_name, "add");
    assert_eq!(report.library_exports[0].invocation, "node require");
    assert!(output_dir.path().join("demo.node").is_file(), "扩展产物必须按 `.node` 命名写出");
}
