use nyar_jit::*;

#[test]
fn baseline_deopt_covers_each_safepoint() {
    let map = build_baseline_deopt_map(2, 3, &[1, 1, 4]);
    assert_eq!(map.function_index, 2);
    assert_eq!(map.entries.len(), 2);
    let entry = map.entry_at(1).expect("entry");
    assert_eq!(entry.frames.len(), 1);
    assert_eq!(entry.frames[0].instruction_index, 2);
    assert_eq!(entry.frames[0].local_count, 3);
}

#[test]
fn materialize_fills_absent_locals_to_local_count() {
    let map = build_baseline_deopt_map(0, 3, &[10]);
    let entry = map.entry_at(10).expect("entry");
    let frames = materialize_interpreter_frames(entry, &[vec![Some(vec![1, 2])]]).expect("ok");
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].instruction_index, 11);
    assert_eq!(frames[0].locals.len(), 3);
    assert_eq!(frames[0].locals[0], RestoredLocal::Provided(vec![1, 2]));
    assert_eq!(frames[0].locals[1], RestoredLocal::Absent);
    assert_eq!(frames[0].locals[2], RestoredLocal::Absent);
}

#[test]
fn materialize_rejects_local_overflow() {
    let map = build_baseline_deopt_map(0, 1, &[0]);
    let entry = map.entry_at(0).expect("entry");
    let err = materialize_interpreter_frames(entry, &[vec![None, None]]).expect_err("overflow");
    assert!(matches!(err, DeoptRestoreError::LocalOverflow { .. }));
}

#[test]
fn inline_deopt_map_preserves_outer_resume_pcs() {
    let map = build_inline_deopt_map(
        1,
        &[
            InlineFrameSpec { function_index: 1, local_count: 2, resume_instruction: None },
            InlineFrameSpec { function_index: 0, local_count: 3, resume_instruction: Some(99) },
        ],
        &[10],
    );
    let entry = map.entry_at(10).expect("entry");
    assert_eq!(entry.frames.len(), 2);
    assert_eq!(entry.frames[0].function_index, 1);
    assert_eq!(entry.frames[0].instruction_index, 11);
    assert_eq!(entry.frames[0].local_count, 2);
    assert_eq!(entry.frames[1].function_index, 0);
    assert_eq!(entry.frames[1].instruction_index, 99);
    assert_eq!(entry.frames[1].local_count, 3);
}
