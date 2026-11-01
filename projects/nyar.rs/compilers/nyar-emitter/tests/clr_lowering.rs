//! CLR 最小 MIR → `MSIL` 模块 lowering 的结构化验证。
//!
//! 这些用例直接构造已冻结的片段计划，验证冻结 operator 落到单条 MSIL opcode、
//! 比较不展开为分支序列，以及未冻结绑定 / 未支持指令的结构化拒绝；不经过前端。

use std::collections::BTreeMap;

use nyar::{ExternalImportLink, Identifier, QualifiedName};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    nyar_backend_clr::{MethodBodyEncoder, MsilMethodSignature, MsilModule, MsilOpcode, MsilType},
    target_operator::{TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe},
    testing::lower_fragment_to_clr_module,
    transitional::binary::pe::{ClrImageKind, PeWriter, PeWriterOptions},
};
use nyar_types::{
    Block, BlockRef, ExecutableFunction, ImportIndex, Instruction, InstructionKind, ItemInstanceId, NyarType, Operand, Terminator, ValueRef,
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
        InstructionKind::Call {
            callee: Operand::Item(operator),
            arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))],
        },
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
        MsilMethodSignature::new(
            MsilType::Int64 { signed: true },
            vec![MsilType::Int64 { signed: true }, MsilType::Int64 { signed: true }]
        )
    );
    assert!(method.instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Add), "{:?}", method.instructions);
    assert!(method.instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Ret));

    let body = MethodBodyEncoder::encode(method, 0x1100_0001).expect("method body encoding");
    assert!(!body.is_empty());

    let pe_bytes = write_library_bytes(&module);
    assert!(pe_bytes.starts_with(b"MZ"), "PE writer must produce a DOS header");
}

#[test]
fn comparison_lowers_to_single_clt_without_branch_expansion() {
    let module = lower_single_operator_call("less", "i64.lt", MsilOpcode::Clt, TargetOperandType::I32, BOOLEAN);
    let instructions = module.global_methods.iter().find(|method| method.method.name == "less").expect("less method").instructions.as_slice();
    assert!(instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Clt));
    assert!(
        !instructions
            .iter()
            .any(|instruction| matches!(instruction.opcode, MsilOpcode::Br | MsilOpcode::Brfalse | MsilOpcode::Brtrue)),
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
        InstructionKind::Call {
            callee: Operand::Item(operator),
            arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))],
        },
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
    fixture.blocks[0].terminator = Terminator::Branch {
        condition: Operand::Value(ValueRef(1)),
        then_target: BlockRef(1),
        else_target: BlockRef(2),
    };

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
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Cast { source: Operand::Value(ValueRef(0)), ty: LONG },
        vec![ValueRef(1)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main, fixture)]));
    plan.testing_module_name("clr_lowering");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());

    let error = lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect_err("Cast must fail");
    assert!(error.contains("尚不支持指令 `Cast`"), "{error}");
}
