//! Semantic MIR 提交门对一等函数值 callee 的放行合同测试。
//!
//! 验收面：`Call { callee: Operand::Value }` 仅当该 SSA 值的冻结类型事实为
//! `NyarType::Function` 时通过 `validate_submission`；非 Function 类型或缺少
//! 冻结类型事实的 Value callee 仍拒绝 SMIR003；`Item` callee 直接调用合同不变。
//! 正向用例走完整生产管线（canonical → target plan → submission gate），
//! 禁止按名字、题号或 target 特判。

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName, TheoryBundle};
use nyar_emitter::{
    CompiledProgram, FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    validate_submission,
};
use nyar_types::{
    CanonicalBlock, CanonicalBlockId, CanonicalCallee, CanonicalFragment, CanonicalFunction, CanonicalInstruction, CanonicalOperation,
    CanonicalPrimitiveType, CanonicalProgram, CanonicalSemanticMir, CanonicalTerminator, CanonicalTypeKind, InstructionId, ItemId, ItemInstanceId,
    ItemInstanceRecord, LinkedSemanticProgram, MirValueId, NyarFunctionType, NyarType, TypeId, TypeRecord, ValueIdentity,
    layout_choice::{InvokeLowering, RepresentationPlan, ValueRepresentation},
};

const FRAGMENT: &str = "smir_function_value_callee_fragment";

const I32: NyarType = NyarType::Integer32 { signed: true };

fn function_type(params: Vec<NyarType>, return_type: NyarType) -> NyarType {
    NyarType::Function(Box::new(NyarFunctionType { params, return_type }))
}

/// 从 terminator 推断 `return_type`，构造最小可提交函数（与既有合同测试同一夹具形态）。
fn fixture_function(symbol: &str, instructions: Vec<Instruction>, terminator: Terminator, value_types: BTreeMap<ValueRef, NyarType>) -> ExecutableFunction {
    let return_type = match &terminator {
        Terminator::Return { value: Some(Operand::Value(value)) } => value_types.get(value).cloned().unwrap_or(NyarType::Unit),
        _ => NyarType::Unit,
    };
    ExecutableFunction {
        symbol: symbol.to_string(),
        return_type,
        param_types: Vec::new(),
        value_types,
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block { id: BlockRef(0), label: "entry".to_string(), parameters: Vec::new(), instructions, terminator }],
        diagnostics: Vec::new(),
    }
}

fn instr(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    Instruction { results, ..Instruction::from_kind(kind) }
}

/// `apply(f, x) -> i32 { return f(x) }` 的 canonical 形状（参照 target plan 合同测试的构造）：
/// callee 是携带冻结 Function 类型事实的 SSA 值，配 `InvokeLowering::TypedReference`。
fn linked_base() -> LinkedSemanticProgram {
    let mut linked = LinkedSemanticProgram::default();
    linked.module_name = "smir-function-value".into();
    let value_type = TypeId::from_index(1).unwrap();
    linked.types.insert(value_type, TypeRecord { declaration: value_type, kind: CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 32, signed: true }) });
    let fragment_id = Identifier::new(FRAGMENT);
    linked.fragments.insert(fragment_id.clone(), CanonicalFragment { id: fragment_id, ..Default::default() });
    linked
}

fn function_value_program() -> CanonicalProgram {
    let mut linked = linked_base();
    let caller = ItemInstanceId::from_index(0).unwrap();
    let function_type_id = TypeId::from_index(0).unwrap();
    let value_type = TypeId::from_index(1).unwrap();
    linked.types.insert(
        function_type_id,
        TypeRecord { declaration: function_type_id, kind: CanonicalTypeKind::Function { parameters: vec![value_type], return_type: value_type } },
    );
    linked.item_instances.insert(
        caller,
        ItemInstanceRecord { declaration: ItemId::from_index(0).unwrap(), parameter_types: vec![function_type_id, value_type], return_type: value_type },
    );
    linked.callable_names.insert(caller, QualifiedName::new(vec![Identifier::new("apply")]));
    let function_value = MirValueId::from_index(0).unwrap();
    let argument = MirValueId::from_index(1).unwrap();
    let result = MirValueId::from_index(2).unwrap();
    let function = CanonicalFunction {
        instance: caller,
        parameters: vec![(function_value, function_type_id), (argument, value_type)],
        parameter_names: vec!["function_value".to_string(), "argument".to_string()],
        return_type: value_type,
        value_types: BTreeMap::from([(function_value, function_type_id), (argument, value_type), (result, value_type)]),
        entry: CanonicalBlockId(0),
        blocks: BTreeMap::from([(
            CanonicalBlockId(0),
            CanonicalBlock {
                id: CanonicalBlockId(0),
                parameters: Vec::new(),
                instructions: vec![CanonicalInstruction {
                    id: InstructionId::from_index(0).unwrap(),
                    results: vec![result],
                    operation: CanonicalOperation::Invoke { callee: CanonicalCallee::Value(function_value), arguments: vec![argument] },
                }],
                terminator: CanonicalTerminator::Return { value: Some(result) },
            },
        )]),
    };
    CanonicalProgram { linked, mir: CanonicalSemanticMir { module_name: "smir-function-value".into(), functions: BTreeMap::from([(caller, function)]) } }
}

fn production_submission(canonical: &CanonicalProgram) -> FragmentSubmission {
    let mut plan = RepresentationPlan::default();
    for function in canonical.mir.functions.values() {
        for value in function.value_types.keys() {
            plan.value_reps.insert(ValueIdentity::new(function.instance, *value), ValueRepresentation::Specialized);
        }
    }
    plan.invoke_lowerings.insert(InstructionId::from_index(0).unwrap(), InvokeLowering::TypedReference);
    let program = CompiledProgram::new(canonical.clone(), plan).expect("测试表示输入必须一致");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = Identifier::new(FRAGMENT);
    let executable = TargetExecutionPlan::from_compiled_program(&program, &fragment, TheoryBundle::default(), &roots).expect("`TypedReference` 函数值调用必须成立目标私有调用合同");
    FragmentSubmission::from_backend_plan(executable)
}

#[test]
fn function_value_callee_passes_submission_gate_through_production_pipeline() {
    let canonical = function_value_program();
    canonical.validate().expect("函数值调用必须通过 canonical 合同");
    let submission = production_submission(&canonical);
    let result = validate_submission(&submission);
    assert!(result.is_ok(), "Function-SSA `Value` callee 必须通过 Semantic MIR 提交门: {:?}", result.err());
}

/// `indirect(x) -> i32 { return f(x) }`，`f` 的冻结 SSA 类型由测试注入。
fn value_callee_function(callee_type: Option<NyarType>) -> ExecutableFunction {
    let callee = ValueRef(0);
    let argument = ValueRef(1);
    let result = ValueRef(2);
    let mut value_types = BTreeMap::from([(argument, I32), (result, I32)]);
    if let Some(ty) = callee_type {
        value_types.insert(callee, ty);
    }
    fixture_function(
        "indirect",
        vec![instr(InstructionKind::Call { callee: Operand::Value(callee), arguments: vec![Operand::Value(argument)] }, vec![result])],
        Terminator::Return { value: Some(Operand::Value(result)) },
        value_types,
    )
}

fn submission_with(caller_instance: ItemInstanceId, caller: ExecutableFunction) -> FragmentSubmission {
    let mut plan = TargetExecutionPlan::default();
    plan.testing_functions_mut().insert(caller_instance, caller);
    FragmentSubmission::from_backend_plan(plan)
}

#[test]
fn non_function_value_callee_rejected_smir003() {
    let caller = ItemInstanceId::from_index(0).unwrap();
    let submission = submission_with(caller, value_callee_function(Some(I32)));
    let error = validate_submission(&submission).expect_err("非 Function 类型 `Value` callee 不得放行");
    assert_eq!(error.code, "SMIR003");
    assert_eq!(error.function, "indirect");
}

#[test]
fn value_callee_without_frozen_ssa_type_rejected_smir003() {
    let caller = ItemInstanceId::from_index(0).unwrap();
    let submission = submission_with(caller, value_callee_function(None));
    let error = validate_submission(&submission).expect_err("缺少冻结类型事实的 `Value` callee 必须失败闭合");
    assert_eq!(error.code, "SMIR003");
}

#[test]
fn constant_callee_rejected_smir003() {
    let callee = ValueRef(0);
    let result = ValueRef(2);
    let function = fixture_function(
        "constant_callee",
        vec![instr(InstructionKind::Call { callee: Operand::Constant(Constant::Int(7)), arguments: Vec::new() }, vec![result])],
        Terminator::Return { value: Some(Operand::Value(result)) },
        BTreeMap::from([(callee, function_type(vec![I32], I32)), (result, I32)]),
    );
    let submission = submission_with(ItemInstanceId::from_index(0).unwrap(), function);
    let error = validate_submission(&submission).expect_err("`Constant` callee 不得放行");
    assert_eq!(error.code, "SMIR003");
}

/// `call_identity(x) -> i32 { return identity(x) }` + `identity(v) -> i32`：
/// `Item` callee 直接调用合同保持不变。
fn item_call_functions() -> (ItemInstanceId, ItemInstanceId, ExecutableFunction, ExecutableFunction) {
    let caller_instance = ItemInstanceId::from_index(0).unwrap();
    let identity_instance = ItemInstanceId::from_index(1).unwrap();
    let argument = ValueRef(0);
    let result = ValueRef(1);
    let caller = fixture_function(
        "call_identity",
        vec![instr(InstructionKind::Call { callee: Operand::Item(identity_instance), arguments: vec![Operand::Value(argument)] }, vec![result])],
        Terminator::Return { value: Some(Operand::Value(result)) },
        BTreeMap::from([(argument, I32), (result, I32)]),
    );
    let identity = fixture_function(
        "identity",
        Vec::new(),
        Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        BTreeMap::from([(ValueRef(0), I32)]),
    );
    (caller_instance, identity_instance, caller, identity)
}

#[test]
fn item_callee_direct_call_contract_is_unchanged() {
    let (caller_instance, identity_instance, caller, identity) = item_call_functions();
    let mut plan = TargetExecutionPlan::default();
    plan.testing_functions_mut().insert(caller_instance, caller);
    plan.testing_functions_mut().insert(identity_instance, identity);
    let submission = FragmentSubmission::from_backend_plan(plan);
    assert!(validate_submission(&submission).is_ok(), "`Item` callee 直接调用必须继续通过提交门");
}

#[test]
fn unresolved_item_callee_still_rejected_smir003() {
    let (caller_instance, _identity_instance, caller, _identity) = item_call_functions();
    let submission = submission_with(caller_instance, caller);
    let error = validate_submission(&submission).expect_err("未登记 `Item` callee 仍须拒绝");
    assert_eq!(error.code, "SMIR003");
    assert_eq!(error.function, "call_identity");
}
