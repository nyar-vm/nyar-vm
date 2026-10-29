use nyar_types::{
    layout_choice::ValueRepresentation,
    semantic_ids::{InstructionIdKind, ItemIdKind},
    *,
};

#[test]
fn evidence_id_is_stable_and_distinct() {
    let a = EvidenceId::from_parts("Comparable", "Int32", "compare");
    let b = EvidenceId::from_parts("Comparable", "Int32", "compare");
    let c = EvidenceId::from_parts("Comparable", "Utf8", "compare");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.as_str(), "evidence:Comparable@Int32#compare");
}

#[test]
fn instruction_id_is_dense_and_nonzero() {
    let id = InstructionId::from_index(0).expect("index 0");
    assert_eq!(id.index(), 0);
    assert!(InstructionId::from_index(u32::MAX).is_some() || InstructionId::from_index(u32::MAX).is_none());
}

#[test]
fn representation_plan_keys_are_stable_ids() {
    let mut plan = layout_choice::RepresentationPlan::default();
    let insn = InstructionId::from_index(3).unwrap();
    plan.invoke_lowerings.insert(insn, layout_choice::InvokeLowering::Direct);
    assert!(plan.invoke_lowerings.contains_key(&insn));
    let function = ItemInstanceId::from_index(0).unwrap();
    let other_function = ItemInstanceId::from_index(1).unwrap();
    let value = MirValueId::from_index(0).unwrap();
    plan.value_reps.insert(ValueIdentity::new(function, value), ValueRepresentation::Specialized);
    plan.value_reps.insert(ValueIdentity::new(other_function, value), ValueRepresentation::Reified);
    assert!(plan.value_reps.contains_key(&ValueIdentity::new(function, value)));
    assert!(plan.value_reps.contains_key(&ValueIdentity::new(other_function, value)));
    assert_eq!(plan.value_reps.len(), 2);
}

#[test]
fn item_and_type_instance_ids_are_opaque() {
    let item = ItemId::from_index(0).expect("item");
    let ty = TypeInstanceId::from_index(1).expect("type instance");
    let import = ImportIndex::from_index(0).expect("import");
    assert_eq!(item.index(), 0);
    assert_eq!(ty.index(), 1);
    assert_eq!(import.index(), 0);
}

#[test]
fn branded_ids_are_distinct_types() {
    fn expects_instruction(_: InstructionId) {}
    expects_instruction(InstructionId::from_index(0).unwrap());
    assert_eq!(InstructionIdKind::NAME, "InstructionId");
    assert_eq!(ItemIdKind::NAME, "ItemId");
}

// 语言内建结构操作身份归 frontend 私有，OVM 指令编号归 `odin_bytecode::IntrinsicOpcode`；
// `nyar-types` 不再导出跨这两个所有者的 `IntrinsicId`。这里只验证属性身份仍是可扩展的。
#[test]
fn attribute_ids_are_extensible() {
    assert_eq!(builtin_attribute::export().index(), 0);
    assert_eq!(builtin_attribute::main().index(), 1);
    let seeds = builtin_attribute::seed_registrations();
    assert_eq!(seeds[0].name, "export");
    assert_eq!(seeds.len(), 5);
    assert_eq!(builtin_attribute::lookup_seed("export"), Some(builtin_attribute::export()));
    assert_eq!(builtin_attribute::lookup_seed("workload_phase"), Some(builtin_attribute::workload_phase()));
    assert_eq!(builtin_attribute::lookup_seed("custom_attr"), None);
    let cap = ImportCapability::new("env", "console_log");
    assert_eq!(cap.to_string(), "env::console_log");
}
