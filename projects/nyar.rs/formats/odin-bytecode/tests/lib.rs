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
fn unsigned_comparison_opcodes_round_trip() {
    for (byte, code) in
        [(0x43u8, OdinHeadCode::I32LtU), (0x45u8, OdinHeadCode::I32LeU), (0x47u8, OdinHeadCode::I32GtU), (0x49u8, OdinHeadCode::I32GeU)]
    {
        assert_eq!(OdinHeadCode::from_u8(byte), Some(code), "槽位 0x{byte:02X} 必须解析为 {code:?}");
        assert_eq!(code as u8, byte);
        assert_eq!(code.form(), OdinInstructionForm::Plain);
        assert_eq!(code.code_size(), 1);
    }

    let mut stream = Vec::new();
    for code in [OdinHeadCode::I32LtU, OdinHeadCode::I32LeU, OdinHeadCode::I32GtU, OdinHeadCode::I32GeU] {
        emit_plain(&mut stream, code);
    }
    for (index, code) in [OdinHeadCode::I32LtU, OdinHeadCode::I32LeU, OdinHeadCode::I32GtU, OdinHeadCode::I32GeU].iter().enumerate() {
        let decoded = decode_at(&stream, index);
        assert_eq!(decoded.code, *code);
        assert!(decoded.is_valid(), "{code:?} 必须是可解码的 Plain 指令");
    }
}

#[test]
fn bitwise_opcodes_round_trip() {
    for (byte, code) in [
        (0x36u8, OdinHeadCode::I32And),
        (0x37u8, OdinHeadCode::I32Or),
        (0x38u8, OdinHeadCode::I32Xor),
        (0x39u8, OdinHeadCode::I32Shl),
        (0x3Au8, OdinHeadCode::I32ShrS),
        (0x3Bu8, OdinHeadCode::I32ShrU),
    ] {
        assert_eq!(OdinHeadCode::from_u8(byte), Some(code), "槽位 0x{byte:02X} 必须解析为 {code:?}");
        assert_eq!(code as u8, byte);
        assert_eq!(code.form(), OdinInstructionForm::Plain);
        assert_eq!(code.code_size(), 1);
    }

    let mut stream = Vec::new();
    for code in
        [OdinHeadCode::I32And, OdinHeadCode::I32Or, OdinHeadCode::I32Xor, OdinHeadCode::I32Shl, OdinHeadCode::I32ShrS, OdinHeadCode::I32ShrU]
    {
        emit_plain(&mut stream, code);
    }
    for (index, code) in
        [OdinHeadCode::I32And, OdinHeadCode::I32Or, OdinHeadCode::I32Xor, OdinHeadCode::I32Shl, OdinHeadCode::I32ShrS, OdinHeadCode::I32ShrU]
            .iter()
            .enumerate()
    {
        let decoded = decode_at(&stream, index);
        assert_eq!(decoded.code, *code);
        assert!(decoded.is_valid(), "{code:?} 必须是可解码的 Plain 指令");
    }
}

#[test]
fn bitwise_opcode_gaps_stay_unallocated() {
    // `0x34` 留给未来 DivU/RemU，`0x3C..0x3F` 未分配；一律 fail-closed。
    for byte in [0x34u8, 0x3C, 0x3D, 0x3E, 0x3F] {
        assert_eq!(OdinHeadCode::from_u8(byte), None, "未分配槽位 0x{byte:02X} 必须拒绝解码");
    }
}

#[test]
fn signed_comparison_slots_stay_signed_and_unallocated_gaps_rejected() {
    // 偶数槽位属于有符号变体，奇数槽位属于无符号变体；`0x49` 之后不得静默分配。
    assert_eq!(OdinHeadCode::from_u8(0x42), Some(OdinHeadCode::I32LtS));
    assert_eq!(OdinHeadCode::from_u8(0x44), Some(OdinHeadCode::I32LeS));
    assert_eq!(OdinHeadCode::from_u8(0x46), Some(OdinHeadCode::I32GtS));
    assert_eq!(OdinHeadCode::from_u8(0x48), Some(OdinHeadCode::I32GeS));
    assert_eq!(OdinHeadCode::from_u8(0x4A), None, "未分配槽位必须拒绝解码");
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
