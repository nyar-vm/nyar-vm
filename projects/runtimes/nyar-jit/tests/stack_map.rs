use nyar_jit::*;

#[test]
fn conservative_maps_cover_all_locals_at_each_safepoint() {
    let maps = build_conservative_stack_maps(3, 4, &[0, 2, 2, 5]);
    assert_eq!(maps.function_index, 3);
    assert_eq!(maps.entries.len(), 3);
    assert_eq!(maps.entries[0].local_root_slots, vec![0, 1, 2, 3]);
    assert!(maps.entry_at(2).is_some());
    assert!(maps.entry_at(1).is_none());
}
