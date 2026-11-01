use nyar_gc::ObjectHeap;
use odin_bytecode::OdinImport;
use odin_vm::{
    host::{HostOp, ResolvedImport, execute_host_op, parse_host_op, resolve_import},
    value::Value,
};

#[test]
fn parses_all_seed_host_ops() {
    assert!(parse_host_op("alloc_record").is_none(), "struct ops must not revive string host bridge");
    assert_eq!(parse_host_op("print"), Some(HostOp::Print));
    assert_eq!(parse_host_op("i64_add"), Some(HostOp::I64Add));
    assert_eq!(parse_host_op("begin_phase"), Some(HostOp::BeginPhase));
    assert_eq!(parse_host_op("end_phase"), Some(HostOp::EndPhase));
    assert_eq!(parse_host_op("array.new"), Some(HostOp::ArrayNew));
    assert_eq!(parse_host_op("array.length"), Some(HostOp::ArrayLength));
    assert_eq!(parse_host_op("array.get"), Some(HostOp::ArrayGet));
    assert_eq!(parse_host_op("array.set"), Some(HostOp::ArraySet));
    assert!(parse_host_op("not_a_real_host_op").is_none());
}

#[test]
fn array_host_ops_round_trip_new_length_get_set() {
    let mut heap = ObjectHeap::new();
    let array = execute_host_op(HostOp::ArrayNew, &[Value::I32(3)], &mut heap).expect("array.new");
    assert_eq!(execute_host_op(HostOp::ArrayLength, &[array.clone()], &mut heap).expect("array.length"), Value::I32(3));
    assert_eq!(execute_host_op(HostOp::ArrayGet, &[array.clone(), Value::I32(0)], &mut heap).expect("array.get 初始为 Null"), Value::Null);

    execute_host_op(HostOp::ArraySet, &[array.clone(), Value::I32(1), Value::I64(42)], &mut heap).expect("array.set");
    assert_eq!(execute_host_op(HostOp::ArrayGet, &[array.clone(), Value::I32(1)], &mut heap).expect("array.get"), Value::I64(42));
    assert_eq!(execute_host_op(HostOp::ArrayGet, &[array.clone(), Value::I32(9)], &mut heap).expect("越界读为 Null"), Value::Null);

    // 越界写必须结构化失败，不得静默扩容。
    assert!(execute_host_op(HostOp::ArraySet, &[array.clone(), Value::I32(9), Value::I64(1)], &mut heap).is_err(), "越界写必须拒绝");
    // 负长度分配必须拒绝。
    assert!(execute_host_op(HostOp::ArrayNew, &[Value::I32(-1)], &mut heap).is_err(), "负长度不得分配");
    // 非布局对象不得当成数组。
    let scalar = Value::I32(7);
    assert!(execute_host_op(HostOp::ArrayLength, &[scalar.clone()], &mut heap).is_err(), "标量不得通过数组长度路径");
}

#[test]
fn unknown_array_symbol_stays_unresolved_at_load() {
    let import = OdinImport { kind: odin_bytecode::OdinImportKind::Function, module_name: "nyar.host".into(), symbol_name: "array.foo".into() };
    assert!(resolve_import(&import).is_err(), "未登记的 `nyar.host` 符号必须在加载期拒绝，不得静默外部化");
}

#[test]
fn begin_and_end_phase_update_heap_strategy() {
    let mut heap = ObjectHeap::new();
    execute_host_op(HostOp::BeginPhase, &[Value::String("request".into()), Value::I32(5)], &mut heap).expect("begin");
    assert_eq!(heap.strategy().phase_depth(), 1);
    assert_eq!(heap.policy().hints.phase.as_deref(), Some("request"));
    assert_eq!(heap.policy().hints.pause_budget_ms, Some(5));
    execute_host_op(HostOp::EndPhase, &[Value::String("request".into())], &mut heap).expect("end");
    assert_eq!(heap.strategy().phase_depth(), 0);
}

#[test]
fn resolves_non_host_as_external() {
    let import = OdinImport { kind: odin_bytecode::OdinImportKind::Function, module_name: "guest.mod".into(), symbol_name: "whatever".into() };
    assert_eq!(resolve_import(&import).unwrap(), ResolvedImport::External);
}
