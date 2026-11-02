//! CLR 最小 MIR → `MSIL` 模块 lowering 的结构化验证。
//!
//! 这些用例直接构造已冻结的片段计划，验证冻结 operator 落到单条 MSIL opcode、
//! 比较不展开为分支序列，以及未冻结绑定 / 未支持指令的结构化拒绝；不经过前端。

use std::collections::BTreeMap;

use nyar::{ExternalImportLink, Identifier, QualifiedName};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    nyar_backend_clr::{MethodBodyEncoder, MsilMethodSignature, MsilModule, MsilOpcode, MsilType},
    target_operator::{
        TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe,
    },
    testing::lower_fragment_to_clr_module,
    transitional::binary::pe::{ClrImageKind, PeWriter, PeWriterOptions},
};
use nyar_types::{
    AggregateLayout, AggregateLayoutPlan, Block, BlockRef, ExecutableFunction, FieldLayout, ImportIndex, Instruction, InstructionKind,
    ItemInstanceId, NyarType, Operand, StorageKind, Terminator, ValueRef,
};

const LONG: NyarType = NyarType::Integer64 { signed: true };
const BOOLEAN: NyarType = NyarType::Boolean;

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

fn operator_import(mnemonic: &str, plan: TargetOperatorPlan) -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(0).expect("import index"),
        link: ExternalImportLink::host(Some(Identifier::new("clr-operator")), vec![mnemonic.to_string()]),
        parameter_types: vec![LONG, LONG],
        return_type: LONG,
        target_operator: Some(TargetOperatorBinding::Frozen(plan)),
    }
}

fn operator_plan(encoding: MsilOpcode, operand_type: TargetOperandType, result_type: TargetOperandType) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Clr(encoding),
    }
}

/// `i64` 移位的冻结配方：发射实参后先 `conv.i4` 截窄位移量，再执行 `shl`/`shr`。
const I64_SHIFT_STEPS: &[TargetOperandStep] = &[TargetOperandStep::Arguments, TargetOperandStep::NarrowTopI64ToI32];

fn shift_operator_plan(encoding: MsilOpcode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type: TargetOperandType::I64,
        result_type: TargetOperandType::I64,
        recipe: TargetOperatorRecipe::Steps(I64_SHIFT_STEPS),
        encoding: TargetOperatorEncoding::Clr(encoding),
    }
}

/// 构造只调用一个任意冻结配方 operator 的导出函数（结果载体 `I64`），返回降低后的 MSIL 模块。
fn lower_operator_call_with_plan(name: &str, mnemonic: &str, operator: TargetOperatorPlan) -> MsilModule {
    let instance = ItemInstanceId::from_index(1).expect("operator instance");
    let mut fixture = function("clr.fixture", LONG, vec![LONG, LONG]);
    fixture.value_types.insert(ValueRef(2), LONG);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call { callee: Operand::Item(instance), arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))] },
        vec![ValueRef(2)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main, fixture)]));
    plan.testing_module_name("clr_lowering");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), name.to_string());
    plan.testing_register_import(instance, operator_import(mnemonic, operator));
    lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect("frozen operator must lower")
}

/// 构造只调用一个冻结 `long` operator 的导出函数，并返回降低后的 MSIL 模块。
fn lower_single_operator_call(
    name: &str,
    mnemonic: &str,
    encoding: MsilOpcode,
    result_type: TargetOperandType,
    return_type: NyarType,
) -> MsilModule {
    let operator = ItemInstanceId::from_index(1).expect("operator instance");
    let result_value_type = match result_type {
        TargetOperandType::I64 => LONG,
        TargetOperandType::I32 => BOOLEAN,
        TargetOperandType::F64 => NyarType::Float64,
    };
    let mut fixture = function("clr.fixture", return_type, vec![LONG, LONG]);
    fixture.value_types.insert(ValueRef(2), result_value_type);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call { callee: Operand::Item(operator), arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))] },
        vec![ValueRef(2)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main, fixture)]));
    plan.testing_module_name("clr_lowering");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), name.to_string());
    plan.testing_register_import(operator, operator_import(mnemonic, operator_plan(encoding, TargetOperandType::I64, result_type)));
    lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect("frozen operator must lower")
}

fn write_library_bytes(module: &MsilModule) -> Vec<u8> {
    PeWriter::new(PeWriterOptions {
        assembly_name: "clr_lowering".to_string(),
        module_name: "clr_lowering".to_string(),
        image_kind: ClrImageKind::DynamicLibrary,
    })
    .write_module(module)
    .expect("CLR library PE encoding")
}

#[test]
fn lowers_frozen_long_add_operator_and_roundtrips_method_body() {
    let module = lower_single_operator_call("add", "i64.add", MsilOpcode::Add, TargetOperandType::I64, LONG);
    let method = module.global_methods.iter().find(|method| method.method.name == "add").expect("add method");
    assert_eq!(
        method.method.signature,
        MsilMethodSignature::new(MsilType::Int64 { signed: true }, vec![MsilType::Int64 { signed: true }, MsilType::Int64 { signed: true }])
    );
    assert!(method.instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Add), "{:?}", method.instructions);
    assert!(method.instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Ret));

    let body = MethodBodyEncoder::encode(method, 0x1100_0001).expect("method body encoding");
    assert!(!body.is_empty());

    let pe_bytes = write_library_bytes(&module);
    assert!(pe_bytes.starts_with(b"MZ"), "PE writer must produce a DOS header");
}

/// `i64.shl` 的冻结配方要求位移量先 `conv.i4` 截窄再 `shl`；backend 不得把
/// `i64` 栈顶直接喂给 `shl`。
#[test]
fn lowers_frozen_i64_shift_recipe_with_conv_i4_before_shl() {
    let module = lower_operator_call_with_plan("shift", "i64.shl", shift_operator_plan(MsilOpcode::Shl));
    let method = module.global_methods.iter().find(|method| method.method.name == "shift").expect("shift method");
    let instructions = method.instructions.as_slice();
    let narrow = instructions.iter().position(|instruction| instruction.opcode == MsilOpcode::ConvI4).expect("配方必须发射 conv.i4 截窄位移量");
    let shift = instructions.iter().position(|instruction| instruction.opcode == MsilOpcode::Shl).expect("配方必须执行 shl");
    assert!(narrow < shift, "conv.i4 必须先于 shl: {instructions:?}");
    let body = MethodBodyEncoder::encode(method, 0x1100_0002).expect("shift method body encoding");
    assert!(!body.is_empty());
}

/// `i64.and` 走同载体二元配方：单条 `and`，不引入截窄。
#[test]
fn lowers_frozen_i64_bitand_operator_to_single_and() {
    let module = lower_single_operator_call("bitand", "i64.and", MsilOpcode::And, TargetOperandType::I64, LONG);
    let instructions =
        module.global_methods.iter().find(|method| method.method.name == "bitand").expect("bitand method").instructions.as_slice();
    assert_eq!(instructions.iter().filter(|instruction| instruction.opcode == MsilOpcode::And).count(), 1, "{instructions:?}");
    assert!(instructions.iter().all(|instruction| instruction.opcode != MsilOpcode::ConvI4), "同载体位运算不得发射截窄: {instructions:?}");
    write_library_bytes(&module);
}

#[test]
fn comparison_lowers_to_single_clt_without_branch_expansion() {
    let module = lower_single_operator_call("less", "i64.lt", MsilOpcode::Clt, TargetOperandType::I32, BOOLEAN);
    let instructions = module.global_methods.iter().find(|method| method.method.name == "less").expect("less method").instructions.as_slice();
    assert!(instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Clt));
    assert!(
        !instructions.iter().any(|instruction| matches!(instruction.opcode, MsilOpcode::Br | MsilOpcode::Brfalse | MsilOpcode::Brtrue)),
        "comparison must not expand into branch sequences: {instructions:?}"
    );
    write_library_bytes(&module);
}

#[test]
fn unfrozen_operator_binding_is_rejected_structurally() {
    let operator = ItemInstanceId::from_index(1).expect("operator instance");
    let mut fixture = function("clr.fixture", LONG, vec![LONG, LONG]);
    fixture.value_types.insert(ValueRef(2), LONG);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call { callee: Operand::Item(operator), arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))] },
        vec![ValueRef(2)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main, fixture)]));
    plan.testing_module_name("clr_lowering");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());
    let mut import = operator_import("i64.add", operator_plan(MsilOpcode::Add, TargetOperandType::I64, TargetOperandType::I64));
    import.target_operator = Some(TargetOperatorBinding::Unfrozen);
    plan.testing_register_import(operator, import);

    let error = lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect_err("unfrozen binding must fail");
    assert!(error.contains("缺少可编码助记符"), "{error}");
}

/// 构造两分支控制流，验证条件分支落到单条 `brfalse` + `br`，两块各自返回。
#[test]
fn conditional_branch_lowers_to_single_false_branch() {
    let mut fixture = function("clr.branch", LONG, vec![LONG, BOOLEAN]);
    fixture.blocks.push(Block {
        id: BlockRef(1),
        label: "then_branch".to_string(),
        parameters: Vec::new(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    });
    fixture.blocks.push(Block {
        id: BlockRef(2),
        label: "else_branch".to_string(),
        parameters: Vec::new(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    });
    fixture.blocks[0].terminator =
        Terminator::Branch { condition: Operand::Value(ValueRef(1)), then_target: BlockRef(1), else_target: BlockRef(2) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main, fixture)]));
    plan.testing_module_name("clr_lowering");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "branch".to_string());
    let module = lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect("conditional branch must lower");

    let instructions =
        module.global_methods.iter().find(|method| method.method.name == "branch").expect("branch method").instructions.as_slice();
    assert_eq!(instructions.iter().filter(|instruction| instruction.opcode == MsilOpcode::Brfalse).count(), 1, "{instructions:?}");
    assert_eq!(instructions.iter().filter(|instruction| instruction.opcode == MsilOpcode::Br).count(), 1, "{instructions:?}");
    assert_eq!(instructions.iter().filter(|instruction| instruction.opcode == MsilOpcode::Ret).count(), 2, "{instructions:?}");
    write_library_bytes(&module);
}

#[test]
fn unsupported_instruction_is_rejected_structurally() {
    let mut fixture = function("clr.fixture", LONG, vec![LONG]);
    fixture.value_types.insert(ValueRef(1), LONG);
    fixture.blocks[0].instructions.push(instr(InstructionKind::Cast { source: Operand::Value(ValueRef(0)), ty: LONG }, vec![ValueRef(1)]));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main, fixture)]));
    plan.testing_module_name("clr_lowering");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());

    let error = lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect_err("Cast must fail");
    assert!(error.contains("尚不支持指令 `Cast`"), "{error}");
}

/// `Option<char>` 的实例级布局：discriminator + payload 槽，引用存储。
/// 布局身份（`LayoutId` 0）由计划注册表持有，类型只携带身份不携带拼写匹配。
fn option_char_layout() -> AggregateLayout {
    AggregateLayout {
        id: 0,
        name: "Option".to_string(),
        namespace: "core".to_string(),
        key: None,
        storage: StorageKind::Reference,
        size: 8,
        align: 4,
        fields: vec![
            FieldLayout { name: "discriminant".to_string(), ty: NyarType::Integer32 { signed: false }, offset: 0, size: 4, align: 4 },
            FieldLayout { name: "payload".to_string(), ty: NyarType::Character, offset: 4, size: 4, align: 4 },
        ],
    }
}

/// 泛型实例 `Option<char>`：基身份携带已冻结布局，实参代入 `char`。
fn option_char_type() -> NyarType {
    NyarType::Apply(Box::new(NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") }), vec![NyarType::Character])
}

/// 构造签名为 `(Option<char>, char) -> Option<char>` 的入口操作，返回第一个形参。
fn option_flow_plan(parameters: Vec<NyarType>, return_type: NyarType) -> FragmentSubmission {
    let mut fixture = function("clr.option_flow", return_type, parameters);
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("clr_lowering").testing_entry_operation(main);
    plan.testing_aggregate_layouts(AggregateLayoutPlan { layouts: vec![option_char_layout()], ..AggregateLayoutPlan::default() });
    FragmentSubmission::from_backend_plan(plan)
}

/// 泛型实例 `Option<char>` 的 CLR 载体合同：引用存储形状擦除为 `object`，
/// 形参 / 局部 / 返回槽按引用载体搬运，方法体可编码。
#[test]
fn option_char_flow_lowers_with_erased_reference_carrier() {
    let option = option_char_type();
    let submission = option_flow_plan(vec![option.clone(), NyarType::Character], option.clone());
    let module = lower_fragment_to_clr_module(&submission).expect("Option<char> reference carrier must lower");

    let method = module.global_methods.iter().find(|method| method.method.name == "main").expect("entry method");
    assert_eq!(method.method.signature, MsilMethodSignature::new(MsilType::Object, vec![MsilType::Object, MsilType::Char]));
    assert!(method.instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Ret), "{:?}", method.instructions);
    let body = MethodBodyEncoder::encode(method, 0x1100_0001).expect("method body encoding");
    assert!(!body.is_empty());
    write_library_bytes(&module);
}

/// 携带身份的名义类型引用存储时同样走擦除引用载体（不经泛型实参形状）。
#[test]
fn nominal_with_registered_reference_layout_lowers_to_reference_carrier() {
    let nominal = NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") };
    let submission = option_flow_plan(vec![nominal.clone()], nominal);
    let module = lower_fragment_to_clr_module(&submission).expect("reference nominal must lower");
    let method = module.global_methods.iter().find(|method| method.method.name == "main").expect("entry method");
    assert_eq!(
        method.method.signature,
        MsilMethodSignature::new(
            MsilType::Named("nyar::layout::Option$0".to_string()),
            vec![MsilType::Named("nyar::layout::Option$0".to_string())]
        )
    );
    write_library_bytes(&module);
}

/// `Nominal` 携带的布局身份未在计划中注册：合同破坏，必须结构化失败。
#[test]
fn nominal_with_unregistered_layout_is_rejected_structurally() {
    let nominal = NyarType::Nominal { layout: 7, spelling: Identifier::new("Option") };
    let submission = option_flow_plan(vec![nominal.clone()], nominal);
    let error = lower_fragment_to_clr_module(&submission).expect_err("unregistered layout identity must fail");
    assert!(error.contains("未在计划中注册"), "{error}");
}

/// 值存储聚合同样指向布局类型定义：CLR 没有内联结构体，逐字段拷贝由
/// `AggregateCopy` 承担，是唯一允许的复制路径。
#[test]
fn value_storage_nominal_uses_layout_type_name() {
    let mut layout = option_char_layout();
    layout.storage = StorageKind::Value;
    let nominal = NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") };
    let mut fixture = function("clr.option_flow", nominal.clone(), vec![nominal.clone()]);
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("clr_lowering").testing_entry_operation(main);
    plan.testing_aggregate_layouts(AggregateLayoutPlan { layouts: vec![layout], ..AggregateLayoutPlan::default() });

    let module = lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect("value aggregate must lower");
    let method = module.global_methods.iter().find(|method| method.method.name == "main").expect("entry method");
    assert_eq!(
        method.method.signature,
        MsilMethodSignature::new(
            MsilType::Named("nyar::layout::Option$0".to_string()),
            vec![MsilType::Named("nyar::layout::Option$0".to_string())]
        )
    );
}

/// 零字段值存储聚合与 wasm 标量语义一致：没有单槽 CLR 载体，必须结构化失败。
#[test]
fn zero_field_value_storage_nominal_is_rejected_structurally() {
    let mut layout = option_char_layout();
    layout.storage = StorageKind::Value;
    layout.fields.clear();
    let nominal = NyarType::Nominal { layout: 0, spelling: Identifier::new("Option") };
    let mut fixture = function("clr.option_flow", nominal.clone(), vec![nominal.clone()]);
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(0))) };
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("clr_lowering").testing_entry_operation(main);
    plan.testing_aggregate_layouts(AggregateLayoutPlan { layouts: vec![layout], ..AggregateLayoutPlan::default() });

    let error = lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect_err("zero-field value aggregate must fail");
    assert!(error.contains("值存储聚合"), "{error}");
}

/// 裸 `Named` 短名残差不携带身份，仍必须结构化失败。
#[test]
fn bare_named_residual_is_rejected_structurally() {
    let residual = NyarType::Named(Identifier::new("Option"));
    let submission = option_flow_plan(vec![residual.clone()], residual);
    let error = lower_fragment_to_clr_module(&submission).expect_err("bare `Named` residual must fail");
    assert!(error.contains("缺少该形状的 CLR 载体合同"), "{error}");
}
