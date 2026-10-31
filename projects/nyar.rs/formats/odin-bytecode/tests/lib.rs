use odin_bytecode::*;

#[test]
fn structural_opcodes_are_imm1() {
    assert_eq!(OdinHeadCode::ObjectNew.form(), OdinInstructionForm::Imm1);
    assert_eq!(OdinHeadCode::FieldGet.form(), OdinInstructionForm::Imm1);
    assert_eq!(OdinHeadCode::FieldSet.form(), OdinInstructionForm::Imm1);
    assert_eq!(OdinHeadCode::from_u8(0x50), Some(OdinHeadCode::ObjectNew));
    assert_eq!(OdinHeadCode::from_u8(0x51), Some(OdinHeadCode::FieldGet));
    assert_eq!(OdinHeadCode::from_u8(0x52), Some(OdinHeadCode::FieldSet));
}

#[test]
fn bytecode_format_version_tracks_nyar_version() {
    assert_eq!(BYTECODE_FORMAT_VERSION, Odin_VERSION);
    // 未正式发布前格式基线统一为 v0；递增格式必须同时抬 `NYAR_VERSION`。
    assert_eq!(BYTECODE_FORMAT_VERSION, 0);
}

#[test]
fn obsolete_call_native_is_not_a_live_head_code() {
    assert_eq!(OBSOLETE_CALL_NATIVE, 0xD1);
    assert_eq!(OdinHeadCode::from_u8(OBSOLETE_CALL_NATIVE), None);
    assert_ne!(OdinHeadCode::CallImport as u8, OBSOLETE_CALL_NATIVE);
}

#[test]
fn emit_helpers_round_trip_with_decode_at() {
    let mut code = Vec::new();
    emit_plain(&mut code, OdinHeadCode::Return);
    emit_imm1(&mut code, OdinHeadCode::Constant, 7);
    emit_imm2(&mut code, OdinHeadCode::CallImport, 1, 2);
    let ret = decode_at(&code, 0);
    assert_eq!(ret.code, OdinHeadCode::Return);
    assert_eq!(ret.size, 1);
    let c = decode_at(&code, 1);
    assert_eq!(c.code, OdinHeadCode::Constant);
    assert_eq!(c.operand1, 7);
    assert_eq!(c.size, 5);
    let call = decode_at(&code, 6);
    assert_eq!(call.code, OdinHeadCode::CallImport);
    assert_eq!(call.operand1, 1);
    assert_eq!(call.operand2, 2);
    assert_eq!(call.size, 9);
}

#[test]
fn intrinsic_opcode_operand_round_trips() {
    for opcode in [
        IntrinsicOpcode::ArrayPush,
        IntrinsicOpcode::ArrayLen,
        IntrinsicOpcode::ArrayGet,
        IntrinsicOpcode::ArraySet,
        IntrinsicOpcode::RefDeref,
        IntrinsicOpcode::IsNull,
        IntrinsicOpcode::UnwrapNull,
    ] {
        assert_eq!(IntrinsicOpcode::from_operand(opcode.raw()), Ok(opcode), "编号必须稳定往返：{opcode:?}");
    }
    // 编号是 `CallIntrinsic` 操作数编码，属于 VM 私有合同；改动即格式破坏。
    assert_eq!(IntrinsicOpcode::ArrayPush.raw(), 0);
    assert_eq!(IntrinsicOpcode::ArrayLen.raw(), 1);
    assert_eq!(IntrinsicOpcode::ArrayGet.raw(), 2);
    assert_eq!(IntrinsicOpcode::ArraySet.raw(), 3);
    assert_eq!(IntrinsicOpcode::RefDeref.raw(), 4);
    assert_eq!(IntrinsicOpcode::IsNull.raw(), 5);
    assert_eq!(IntrinsicOpcode::UnwrapNull.raw(), 6);
}

#[test]
fn unknown_intrinsic_operand_is_rejected() {
    for operand in [-1, 7, 99] {
        assert!(
            IntrinsicOpcode::from_operand(operand).is_err(),
            "未知 `CallIntrinsic` 操作数 {operand} 必须在运行前拒绝，不能静默当成某个指令"
        );
    }
}

#[test]
fn layouts_section_round_trips() {
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "layout-test".into(),
        constants: Vec::new(),
        functions: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        witness_entries: Vec::new(),
        code_bytes: Vec::new(),
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: vec![OdinLayout { field_count: 2 }, OdinLayout { field_count: 0 }],
    };
    let bytes = encode_module(&data);
    let decoded = decode_module(&bytes).expect("decode");
    assert_eq!(decoded.layouts, data.layouts);
}
