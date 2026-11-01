//! `TypedReference` invoke lowering 的目标私有调用合同消费测试。
//!
//! 验收面：`CanonicalCallee::Value` 配 `InvokeLowering::TypedReference` 且 callee SSA
//! 携带 canonical 冻结的 Function 类型事实时，目标准备必须放行并降为
//! `Call { callee: Operand::Value }`；其余组合（`Value` 配 `Direct`、`Item` 配
//! `TypedReference`、`Item` 配 `TypedWitness`）仍失败闭合，`Item` 配 `Direct` 行为不变。
//! 禁止按名字或题号特判，合同只能由冻结的语义事实成立。

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName, TheoryBundle};
use nyar_emitter::{
    CompiledProgram, TargetExecutionPlan,
    contracts::{InstructionKind, Operand, ValueRef},
};
use nyar_types::{
    CanonicalBlock, CanonicalBlockId, CanonicalCallee, CanonicalFragment, CanonicalFunction, CanonicalInstruction, CanonicalOperation,
    CanonicalPrimitiveType, CanonicalProgram, CanonicalSemanticMir, CanonicalTerminator, CanonicalTypeKind, InstructionId, ItemId,
    ItemInstanceId, ItemInstanceRecord, LinkedSemanticProgram, MirValueId, NyarType, TypeId, TypeRecord, ValueIdentity,
    layout_choice::{InvokeLowering, RepresentationPlan, ValueRepresentation},
};

const FRAGMENT: &str = "typed_reference_fragment";

fn linked_base() -> LinkedSemanticProgram {
    let mut linked = LinkedSemanticProgram::default();
    linked.module_name = "typed-reference".into();
    let value_type = TypeId::from_index(1).unwrap();
    linked.types.insert(value_type, TypeRecord { declaration: value_type, kind: CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 32, signed: true }) });
    let fragment_id = Identifier::new(FRAGMENT);
    linked.fragments.insert(fragment_id.clone(), CanonicalFragment { id: fragment_id, ..Default::default() });
    linked
}

/// `apply(f: fn(i32) -> i32, x: i32) -> i32 { return f(x) }`：
/// callee 是携带冻结 Function 类型事实的 SSA 值。
fn function_value_program() -> CanonicalProgram {
    let mut linked = linked_base();
    let caller = ItemInstanceId::from_index(0).unwrap();
    let function_type = TypeId::from_index(0).unwrap();
    let value_type = TypeId::from_index(1).unwrap();
    linked.types.insert(
        function_type,
        TypeRecord { declaration: function_type, kind: CanonicalTypeKind::Function { parameters: vec![value_type], return_type: value_type } },
    );
    linked.item_instances.insert(
        caller,
        ItemInstanceRecord { declaration: ItemId::from_index(0).unwrap(), parameter_types: vec![function_type, value_type], return_type: value_type },
    );
    linked.callable_names.insert(caller, QualifiedName::new(vec![Identifier::new("apply")]));
    let function_value = MirValueId::from_index(0).unwrap();
    let argument = MirValueId::from_index(1).unwrap();
    let result = MirValueId::from_index(2).unwrap();
    let function = CanonicalFunction {
        instance: caller,
        parameters: vec![(function_value, function_type), (argument, value_type)],
        parameter_names: vec!["function_value".to_string(), "argument".to_string()],
        return_type: value_type,
        value_types: BTreeMap::from([(function_value, function_type), (argument, value_type), (result, value_type)]),
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
    CanonicalProgram { linked, mir: CanonicalSemanticMir { module_name: "typed-reference".into(), functions: BTreeMap::from([(caller, function)]) } }
}

/// `call_identity(x: i32) -> i32 { return identity(x) }` + `identity(v: i32) -> i32`：
/// callee 是已链接 Item 实例，用于既有 `Direct` 合同与错配拒绝的对照组。
fn item_call_program() -> CanonicalProgram {
    let mut linked = linked_base();
    let caller = ItemInstanceId::from_index(0).unwrap();
    let identity = ItemInstanceId::from_index(1).unwrap();
    let value_type = TypeId::from_index(1).unwrap();
    for (instance, declaration) in [(caller, ItemId::from_index(0).unwrap()), (identity, ItemId::from_index(1).unwrap())] {
        linked
            .item_instances
            .insert(instance, ItemInstanceRecord { declaration, parameter_types: vec![value_type], return_type: value_type });
    }
    linked.callable_names.insert(caller, QualifiedName::new(vec![Identifier::new("call_identity")]));
    linked.callable_names.insert(identity, QualifiedName::new(vec![Identifier::new("identity")]));
    let argument = MirValueId::from_index(0).unwrap();
    let result = MirValueId::from_index(1).unwrap();
    let caller_function = CanonicalFunction {
        instance: caller,
        parameters: vec![(argument, value_type)],
        parameter_names: vec!["argument".to_string()],
        return_type: value_type,
        value_types: BTreeMap::from([(argument, value_type), (result, value_type)]),
        entry: CanonicalBlockId(0),
        blocks: BTreeMap::from([(
            CanonicalBlockId(0),
            CanonicalBlock {
                id: CanonicalBlockId(0),
                parameters: Vec::new(),
                instructions: vec![CanonicalInstruction {
                    id: InstructionId::from_index(0).unwrap(),
                    results: vec![result],
                    operation: CanonicalOperation::Invoke { callee: CanonicalCallee::Item(identity), arguments: vec![argument] },
                }],
                terminator: CanonicalTerminator::Return { value: Some(result) },
            },
        )]),
    };
    let identity_value = MirValueId::from_index(0).unwrap();
    let identity_function = CanonicalFunction {
        instance: identity,
        parameters: vec![(identity_value, value_type)],
        parameter_names: vec!["value".to_string()],
        return_type: value_type,
        value_types: BTreeMap::from([(identity_value, value_type)]),
        entry: CanonicalBlockId(0),
        blocks: BTreeMap::from([(
            CanonicalBlockId(0),
            CanonicalBlock {
                id: CanonicalBlockId(0),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: CanonicalTerminator::Return { value: Some(identity_value) },
            },
        )]),
    };
    CanonicalProgram {
        linked,
        mir: CanonicalSemanticMir {
            module_name: "typed-reference".into(),
            functions: BTreeMap::from([(caller, caller_function), (identity, identity_function)]),
        },
    }
}

fn representation(program: &CanonicalProgram, invoke: InvokeLowering) -> RepresentationPlan {
    let mut plan = RepresentationPlan::default();
    for function in program.mir.functions.values() {
        for value in function.value_types.keys() {
            plan.value_reps.insert(ValueIdentity::new(function.instance, *value), ValueRepresentation::Specialized);
        }
    }
    plan.invoke_lowerings.insert(InstructionId::from_index(0).unwrap(), invoke);
    plan
}

fn prepare(program: &CompiledProgram) -> nyar_emitter::Result<TargetExecutionPlan> {
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = Identifier::new(FRAGMENT);
    TargetExecutionPlan::from_compiled_program(program, &fragment, TheoryBundle::default(), &roots)
}

fn compiled(canonical: CanonicalProgram, invoke: InvokeLowering) -> CompiledProgram {
    let plan = representation(&canonical, invoke);
    CompiledProgram::new(canonical, plan).expect("测试表示输入必须一致")
}

fn find_call(plan: &TargetExecutionPlan, instance: &ItemInstanceId) -> (Operand, Vec<Operand>) {
    let function = &plan.testing_functions()[instance];
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match &instruction.kind {
            InstructionKind::Call { callee, arguments } => Some((callee.clone(), arguments.clone())),
            _ => None,
        })
        .expect("函数必须含调用指令")
}

#[test]
fn typed_reference_function_value_callee_lowers_to_value_callee_call() {
    let canonical = function_value_program();
    canonical.validate().expect("函数值调用必须通过 canonical 合同");
    let program = compiled(canonical, InvokeLowering::TypedReference);
    let plan = prepare(&program).expect("`Value` callee 配 `TypedReference` 必须成立目标私有调用合同");
    let caller = ItemInstanceId::from_index(0).unwrap();
    let (callee, arguments) = find_call(&plan, &caller);
    assert_eq!(callee, Operand::Value(ValueRef(0)), "函数值 callee 必须降为按 SSA 值的间接调用");
    assert_eq!(arguments, vec![Operand::Value(ValueRef(1))]);
    let lowered = &plan.testing_functions()[&caller];
    assert!(matches!(lowered.value_types[&ValueRef(0)], NyarType::Function(_)), "callee 值必须携带冻结的 Function 签名供 wasm 反查 typeidx");
}

#[test]
fn direct_lowering_rejects_function_value_callee() {
    let program = compiled(function_value_program(), InvokeLowering::Direct);
    let error = prepare(&program).expect_err("`Value` callee 配 `Direct` 不得复用直接调用编码");
    assert!(error.to_string().contains("目标私有调用合同"), "{error}");
}

#[test]
fn typed_reference_rejects_item_callee() {
    let program = compiled(item_call_program(), InvokeLowering::TypedReference);
    let error = prepare(&program).expect_err("`Item` callee 配 `TypedReference` 不得放行");
    assert!(error.to_string().contains("目标私有调用合同"), "{error}");
}

#[test]
fn typed_witness_lowering_rejects_item_callee() {
    let program = compiled(item_call_program(), InvokeLowering::TypedWitness);
    let error = prepare(&program).expect_err("`Item` callee 配 `TypedWitness` 不得放行");
    assert!(error.to_string().contains("目标私有调用合同"), "{error}");
}

#[test]
fn direct_item_call_contract_is_unchanged() {
    let program = compiled(item_call_program(), InvokeLowering::Direct);
    let plan = prepare(&program).expect("`Item` 配 `Direct` 既有合同必须保持不变");
    let caller = ItemInstanceId::from_index(0).unwrap();
    let identity = ItemInstanceId::from_index(1).unwrap();
    let (callee, arguments) = find_call(&plan, &caller);
    assert_eq!(callee, Operand::Item(identity));
    assert_eq!(arguments, vec![Operand::Value(ValueRef(0))]);
    assert!(plan.testing_functions().contains_key(&identity), "被调实例必须进入可执行闭包");
}
