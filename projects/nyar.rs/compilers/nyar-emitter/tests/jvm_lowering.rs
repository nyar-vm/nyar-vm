//! JVM 最小 MIR → `class` lowering 的结构化验证。
//!
//! 这些用例直接构造已冻结的片段计划，验证冻结 operator 的机械展开、比较标签
//! 唯一性，以及未冻结绑定 / 未支持指令的结构化拒绝；不经过前端。

use std::collections::{BTreeMap, BTreeSet};

use nyar::{ExternalImportLink, Identifier, QualifiedName};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    lowering::backends::jvm::JvmOperatorCode,
    nyar_backend_jvm::{JvmClassFile, JvmInstruction, JvmMethodDescriptor, JvmTypeDescriptor, decode_instructions},
    target_operator::{
        TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe,
    },
    testing::lower_fragment_to_jvm_class,
};
use nyar_types::{
    Block, BlockRef, ExecutableFunction, ImportIndex, Instruction, InstructionKind, ItemInstanceId, NyarType, Operand, Terminator, ValueRef,
};

const LONG: NyarType = NyarType::Integer64 { signed: true };
const INTEGER: NyarType = NyarType::Integer32 { signed: true };
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

fn operator_import(plan: TargetOperatorPlan) -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(0).expect("import index"),
        link: ExternalImportLink::host(Some(Identifier::new("jvm-operator")), vec!["long.divide".to_string()]),
        parameter_types: vec![LONG, LONG],
        return_type: LONG,
        target_operator: Some(TargetOperatorBinding::Frozen(plan)),
    }
}

fn operator_plan(code: JvmOperatorCode, operand_type: TargetOperandType, result_type: TargetOperandType) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Jvm(code),
    }
}

/// 构造只调用一个冻结 `long` operator 的导出函数，并返回降低后的 class。
fn lower_single_operator_call(
    code: JvmOperatorCode,
    result_type: TargetOperandType,
    return_type: NyarType,
) -> JvmClassFile {
    let operator = ItemInstanceId::from_index(1).expect("operator instance");
    let result_value_type = match result_type {
        TargetOperandType::I64 => LONG,
        TargetOperandType::I32 => INTEGER,
        TargetOperandType::F64 => NyarType::Float64,
    };
    let mut fixture = function("jvm.fixture", return_type, vec![LONG, LONG]);
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
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());
    plan.testing_register_import(operator, operator_import(operator_plan(code, TargetOperandType::I64, result_type)));
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_jvm_class(&submission).expect("frozen operator must lower")
}

fn method_code<'a>(class_file: &'a JvmClassFile, name: &str) -> &'a [JvmInstruction] {
    let method = class_file.methods.iter().find(|method| method.name == name).expect("lowered method");
    method.code.as_ref().expect("lowered method code").instructions.as_slice()
}

#[test]
fn lowers_frozen_long_add_operator_and_roundtrips_class_bytes() {
    let class_file = lower_single_operator_call(JvmOperatorCode::LongAdd, TargetOperandType::I64, LONG);
    let method = class_file.methods.iter().find(|method| method.name == "add").expect("add method");
    assert_eq!(method.descriptor, JvmMethodDescriptor::new(vec![JvmTypeDescriptor::Long, JvmTypeDescriptor::Long], JvmTypeDescriptor::Long));
    assert!(method_code(&class_file, "add").iter().any(|instruction| matches!(instruction, JvmInstruction::LAdd)));

    let bytes = class_file.to_bytes().expect("class encoding");
    let decoded_class = JvmClassFile::from_bytes(&bytes).expect("class decoding");
    let index = decoded_class.methods.iter().position(|method| method.name == "add").expect("decoded add method");
    let code = decoded_class.raw_method_code[index].as_ref().expect("decoded method code");
    let decoded = decode_instructions(code, &decoded_class.constant_pool);
    let mnemonics = decoded.iter().map(|instruction| instruction.mnemonic.as_str()).collect::<Vec<_>>();
    assert!(mnemonics.contains(&"ladd"), "decoded={mnemonics:?}");
    assert_eq!(mnemonics.last().copied(), Some("lreturn"), "decoded={mnemonics:?}");
}

#[test]
fn comparison_operator_expansion_uses_unique_method_local_labels() {
    let operator = ItemInstanceId::from_index(1).expect("operator instance");
    let mut fixture = function("jvm.compare", BOOLEAN, vec![LONG, LONG]);
    fixture.value_types.insert(ValueRef(2), INTEGER);
    fixture.value_types.insert(ValueRef(3), INTEGER);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call {
            callee: Operand::Item(operator),
            arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))],
        },
        vec![ValueRef(2)],
    ));
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Call {
            callee: Operand::Item(operator),
            arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))],
        },
        vec![ValueRef(3)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(2))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "less".to_string());
    plan.testing_register_import(
        operator,
        operator_import(operator_plan(JvmOperatorCode::LongCompareLessThan, TargetOperandType::I64, TargetOperandType::I32)),
    );
    let submission = FragmentSubmission::from_backend_plan(plan);
    let class_file = lower_fragment_to_jvm_class(&submission).expect("comparison operator must lower");

    let instructions = method_code(&class_file, "less");
    let labels = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            JvmInstruction::Label(label) => Some(label.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    // 块标签之外，两次比较必须各自展开成两个方法内唯一标签。
    let comparison_labels = labels
        .iter()
        .filter(|label| label.ends_with("comparison_false") || label.ends_with("comparison_end"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(comparison_labels.len(), 4, "two comparisons must each expand into two labels: {labels:?}");
    let unique = labels.iter().collect::<BTreeSet<_>>();
    assert_eq!(unique.len(), labels.len(), "labels must be unique: {labels:?}");
    assert!(labels.iter().all(|label| label != "comparison_false" && label != "comparison_end"), "placeholder labels must be relabeled: {labels:?}");

    let compare_count = instructions.iter().filter(|instruction| matches!(instruction, JvmInstruction::LCmp)).count();
    assert_eq!(compare_count, 2);
    class_file.to_bytes().expect("comparison class encoding");
}

#[test]
fn unfrozen_operator_binding_is_rejected_structurally() {
    let operator = ItemInstanceId::from_index(1).expect("operator instance");
    let mut fixture = function("jvm.fixture", LONG, vec![LONG, LONG]);
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
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());
    let mut import = operator_import(operator_plan(JvmOperatorCode::LongAdd, TargetOperandType::I64, TargetOperandType::I64));
    import.target_operator = Some(TargetOperatorBinding::Unfrozen);
    plan.testing_register_import(operator, import);

    let error = lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect_err("unfrozen binding must fail");
    assert!(error.contains("缺少可编码助记符"), "{error}");
}

/// 构造两分支控制流，验证条件分支落到 `ifeq` + `goto`，两块各自返回。
#[test]
fn conditional_branch_lowers_to_compare_and_jump() {
    let mut fixture = function("jvm.branch", LONG, vec![LONG, BOOLEAN]);
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
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "branch".to_string());
    let class_file = lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect("conditional branch must lower");

    let instructions = method_code(&class_file, "branch");
    assert!(instructions.iter().any(|instruction| matches!(instruction, JvmInstruction::IfEq(_))), "{instructions:?}");
    assert!(instructions.iter().any(|instruction| matches!(instruction, JvmInstruction::Goto(_))), "{instructions:?}");
    class_file.to_bytes().expect("branch class encoding");
}

#[test]
fn unsupported_instruction_is_rejected_structurally() {
    let mut fixture = function("jvm.fixture", LONG, vec![LONG]);
    fixture.value_types.insert(ValueRef(1), LONG);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::Cast { source: Operand::Value(ValueRef(0)), ty: LONG },
        vec![ValueRef(1)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("jvm_lowering").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "add".to_string());

    let error = lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect_err("Cast must fail");
    assert!(error.contains("尚不支持指令 `Cast`"), "{error}");
}
