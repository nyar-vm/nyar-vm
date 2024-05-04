use nyar_gc::*;

#[test]
fn satb_records_overwritten_object_id() {
    let mut barrier = WriteBarrier::new();
    let mut slot = Value::Object(7);
    write_value_slot(&mut barrier, &mut slot, Value::Null, true);
    assert_eq!(barrier.satb_buffer(), &[7]);
    assert_eq!(slot, Value::Null);
}

#[test]
fn satb_inactive_skips_buffer() {
    let mut barrier = WriteBarrier::new();
    let mut slot = Value::Object(7);
    write_value_slot(&mut barrier, &mut slot, Value::Null, false);
    assert!(barrier.satb_buffer().is_empty());
}

#[test]
fn satb_records_overwrites_in_order_on_same_slot() {
    // 单线程协议：同槽连续覆盖时，SATB 按覆盖前序追加旧引用。
    let mut barrier = WriteBarrier::new();
    let mut slot = Value::Object(1);
    write_value_slot(&mut barrier, &mut slot, Value::Object(2), true);
    write_value_slot(&mut barrier, &mut slot, Value::Null, true);
    assert_eq!(barrier.satb_buffer(), &[1, 2]);
    assert_eq!(slot, Value::Null);
}

#[test]
fn satb_interleaved_two_slots_preserves_fifo() {
    let mut barrier = WriteBarrier::new();
    let mut a = Value::Object(10);
    let mut b = Value::Object(20);
    write_value_slot(&mut barrier, &mut a, Value::Object(11), true);
    write_value_slot(&mut barrier, &mut b, Value::Object(21), true);
    write_value_slot(&mut barrier, &mut a, Value::Null, true);
    assert_eq!(barrier.satb_buffer(), &[10, 20, 11]);
}

#[test]
fn remembered_set_records_containers_in_call_order() {
    // 单线程协议：老→年轻边按 record 调用序进入记忆集（可含重复）。
    let mut barrier = WriteBarrier::new();
    barrier.record_old_to_young(1);
    barrier.record_old_to_young(2);
    barrier.record_old_to_young(1);
    assert_eq!(barrier.remembered_set(), &[1, 2, 1]);
}

#[test]
fn remembered_set_interleaved_with_satb_keeps_independent_fifos() {
    let mut barrier = WriteBarrier::new();
    let mut slot = Value::Object(7);
    barrier.record_old_to_young(100);
    write_value_slot(&mut barrier, &mut slot, Value::Object(8), true);
    barrier.record_old_to_young(200);
    write_value_slot(&mut barrier, &mut slot, Value::Null, true);
    assert_eq!(barrier.remembered_set(), &[100, 200]);
    assert_eq!(barrier.satb_buffer(), &[7, 8]);
}

#[test]
fn satb_records_overwritten_coroutine_id() {
    let mut barrier = WriteBarrier::new();
    let mut slot = Value::Coroutine(42);
    write_value_slot(&mut barrier, &mut slot, Value::Null, true);
    assert_eq!(barrier.satb_buffer(), &[42]);
    assert_eq!(slot, Value::Null);
}

#[test]
fn drain_satb_returns_fifo_and_clears_buffer() {
    let mut barrier = WriteBarrier::new();
    let mut a = Value::Object(1);
    let mut b = Value::Coroutine(2);
    write_value_slot(&mut barrier, &mut a, Value::Object(3), true);
    write_value_slot(&mut barrier, &mut b, Value::Null, true);
    assert_eq!(barrier.drain_satb(), vec![1, 2]);
    assert!(barrier.satb_buffer().is_empty());
    assert!(barrier.drain_satb().is_empty());
}
