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
fn parses_i64_bitwise_host_ops() {
    assert_eq!(parse_host_op("i64_and"), Some(HostOp::I64And));
    assert_eq!(parse_host_op("i64_or"), Some(HostOp::I64Or));
    assert_eq!(parse_host_op("i64_xor"), Some(HostOp::I64Xor));
    assert_eq!(parse_host_op("i64_shl"), Some(HostOp::I64Shl));
    assert_eq!(parse_host_op("i64_shr"), Some(HostOp::I64Shr));
    // `i64_shr_u` 是逻辑右移（`u64` 的 `>>`），已登记；未登记的签名变体仍须在加载期拒绝。
    let known = OdinImport { kind: odin_bytecode::OdinImportKind::Function, module_name: "nyar.host".into(), symbol_name: "i64_shr_u".into() };
    assert_eq!(resolve_import(&known).expect("i64_shr_u 已登记"), ResolvedImport::Host(HostOp::I64ShrU));
    let unknown =
        OdinImport { kind: odin_bytecode::OdinImportKind::Function, module_name: "nyar.host".into(), symbol_name: "i64_shr_s".into() };
    assert!(resolve_import(&unknown).is_err(), "未登记的位移符号必须在加载期拒绝，不得静默外部化");
}

#[test]
fn i64_unsigned_less_than_uses_unsigned_bit_patterns() {
    assert_eq!(parse_host_op("i64_lt_u"), Some(HostOp::I64LtU));
    let mut heap = ObjectHeap::new();
    // `-1` 的位型是 `u64::MAX`：无符号语义下不小于 `1`，与有符号 `i64_lt` 结论相反。
    assert_eq!(execute_host_op(HostOp::I64LtU, &[Value::I64(-1), Value::I64(1)], &mut heap).expect("i64_lt_u (-1, 1)"), Value::Bool(false));
    assert_eq!(execute_host_op(HostOp::I64Lt, &[Value::I64(-1), Value::I64(1)], &mut heap).expect("i64_lt (-1, 1)"), Value::Bool(true));
    // 非负操作数上无符号与有符号语义一致。
    assert_eq!(execute_host_op(HostOp::I64LtU, &[Value::I64(3), Value::I64(5)], &mut heap).expect("i64_lt_u (3, 5)"), Value::Bool(true));
    assert_eq!(execute_host_op(HostOp::I64LtU, &[Value::I64(5), Value::I64(3)], &mut heap).expect("i64_lt_u (5, 3)"), Value::Bool(false));
    // 非整数载体参数必须结构化失败，不得静默转换。
    assert!(execute_host_op(HostOp::I64LtU, &[Value::Bool(true), Value::I64(1)], &mut heap).is_err(), "bool 参数不得通过 i64 无符号比较路径");
}

#[test]
fn i64_bitwise_host_ops_execute_with_masked_shifts() {
    let mut heap = ObjectHeap::new();
    assert_eq!(execute_host_op(HostOp::I64And, &[Value::I64(12), Value::I64(10)], &mut heap).expect("i64_and"), Value::I64(8));
    assert_eq!(execute_host_op(HostOp::I64Or, &[Value::I64(12), Value::I64(10)], &mut heap).expect("i64_or"), Value::I64(14));
    assert_eq!(execute_host_op(HostOp::I64Xor, &[Value::I64(12), Value::I64(10)], &mut heap).expect("i64_xor"), Value::I64(6));
    assert_eq!(execute_host_op(HostOp::I64Shl, &[Value::I64(1), Value::I64(63)], &mut heap).expect("i64_shl 63"), Value::I64(1i64 << 63));
    // 位移量按 `& 63` 截断，与 wasm `i64.shl`/JVM `lshl`/MSIL `shl` 掩码语义一致。
    assert_eq!(execute_host_op(HostOp::I64Shl, &[Value::I64(3), Value::I64(65)], &mut heap).expect("i64_shl 65 掩码为 1"), Value::I64(6));
    assert_eq!(
        execute_host_op(HostOp::I64Shr, &[Value::I64(-8), Value::I64(1)], &mut heap).expect("i64_shr"),
        Value::I64(-4),
        "必须是算术右移"
    );
    assert_eq!(execute_host_op(HostOp::I64Shr, &[Value::I64(-1), Value::I64(64)], &mut heap).expect("i64_shr 64 掩码为 0"), Value::I64(-1));
    // 非整数载体参数必须结构化失败，不得静默转换。
    assert!(execute_host_op(HostOp::I64Shr, &[Value::Bool(true), Value::I64(1)], &mut heap).is_err(), "bool 参数不得通过 i64 移位路径");
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
