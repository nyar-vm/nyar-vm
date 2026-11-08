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
    // v1：新增 `Trap` 头码（0x0C）。v2：新增 `Integer64` 常量（0x12）。
    // v3：新增 `CallIndirect` 头码（0x06）。v4：`Function` 值改为两槽闭包对象。
    // v5：执行产物身份由 `.odin` 迁到 `.odin`（magic `NYAR` → `ODIN`）。
    // v6：新增 `I64ReinterpretF64`（0x60）/ `F64ReinterpretI64`（0x61）位重解释头码。
    // v7：新增 f32 独立载体指令（0x62–0x6C）：32 位位重解释、f32 算术/比较与 f32↔f64 转换。
    // `decode_module` 对非当前版本一律 `InvalidVersion` 拒绝，旧缓存产物按缓存未命中处理。
    // 递增格式必须同时抬 `ODIN_VERSION`。
    assert_eq!(BYTECODE_FORMAT_VERSION, 7);
}

#[test]
fn odin_magic_is_a_distinct_artifact_identity() {
    assert_eq!(ODIN_MAGIC, 0x4F44_494E);
    assert_eq!(NYAR_ASSEMBLY_MAGIC, 0x4E59_4152);
    assert_ne!(ODIN_MAGIC, NYAR_ASSEMBLY_MAGIC);
}

#[test]
fn legacy_nyar_magic_is_rejected_structurally() {
    let legacy = [0x4E, 0x59, 0x41, 0x52, 0x00, 0x00, 0x00, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let error = decode_module(&legacy).expect_err("`.nyar` magic must not decode as `.odin`");
    assert_eq!(error, OdinDecodeError::LegacyNyarMagic);
    assert!(error.to_string().contains(".nyar") && error.to_string().contains(".odin"));
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
fn trap_opcode_round_trips_and_low_gaps_stay_rejected() {
    // 0x0C 从"未分配"变为 `Trap`；0x07..0x09、0x0E 仍是未分配槽位，必须 fail-closed。
    assert_eq!(OdinHeadCode::from_u8(0x0C), Some(OdinHeadCode::Trap), "槽位 0x0C 必须是 Trap");
    assert_eq!(OdinHeadCode::Trap as u8, 0x0C);
    assert_eq!(OdinHeadCode::Trap.form(), OdinInstructionForm::Plain);
    assert_eq!(OdinHeadCode::Trap.code_size(), 1);
    for byte in [0x07u8, 0x08, 0x09, 0x0E] {
        assert_eq!(OdinHeadCode::from_u8(byte), None, "未分配槽位 0x{byte:02X} 必须拒绝解码");
    }
}

/// `CallIndirect` 的槽位、形态与 operand 编码必须稳定：operand1 = argc。
#[test]
fn call_indirect_round_trips_as_imm1() {
    assert_eq!(OdinHeadCode::CallIndirect as u8, 0x06);
    assert_eq!(OdinHeadCode::from_u8(0x06), Some(OdinHeadCode::CallIndirect));
    assert_eq!(OdinHeadCode::CallIndirect.form(), OdinInstructionForm::Imm1);
    assert_eq!(OdinHeadCode::CallIndirect.code_size(), 5);

    let mut stream = Vec::new();
    emit_imm1(&mut stream, OdinHeadCode::CallIndirect, 2);
    let decoded = decode_at(&stream, 0);
    assert_eq!(decoded.code, OdinHeadCode::CallIndirect);
    assert_eq!(decoded.operand1, 2);
    assert_eq!(decoded.operand2, 0);
    assert_eq!(decoded.size, 5);
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

fn module_with_constants(constants: Vec<OdinConstant>) -> OdinModuleData {
    OdinModuleData {
        version: Odin_VERSION,
        name: "constant-pool-test".into(),
        constants,
        functions: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        witness_entries: Vec::new(),
        code_bytes: Vec::new(),
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    }
}

/// `Integer64` 必须按 8 字节小端往返；`i64::MAX` / `i64::MIN` 不得在编码层被截成 32 位。
#[test]
fn integer64_constant_round_trips() {
    let constants = vec![
        OdinConstant::Integer32(-1),
        OdinConstant::Integer64(i64::MAX),
        OdinConstant::Integer64(i64::MIN),
        OdinConstant::Integer64(2_147_483_648),
    ];
    let data = module_with_constants(constants.clone());
    let decoded = decode_module(&encode_module(&data)).expect("Integer64 常量必须可往返");
    assert_eq!(decoded.constants, constants, "常量池往返后必须逐项相同");
}

/// NaN payload 与 `-0.0` 的符号位属于位模式，`OdinConstant` 的 `PartialEq` 无法区分，必须按位断言。
#[test]
fn float64_constant_round_trips_bit_exactly() {
    let negative_zero = OdinConstant::Float64(-0.0f64);
    let nan_payload = OdinConstant::Float64(f64::from_bits(0x7ff8_0000_dead_beef));
    let data = module_with_constants(vec![negative_zero.clone(), nan_payload.clone()]);
    let decoded = decode_module(&encode_module(&data)).expect("float64 常量必须可往返");
    match (&decoded.constants[0], &decoded.constants[1]) {
        (OdinConstant::Float64(negative), OdinConstant::Float64(nan)) => {
            assert_eq!(negative.to_bits(), (-0.0f64).to_bits(), "-0.0 的符号位必须保留");
            assert_eq!(nan.to_bits(), 0x7ff8_0000_dead_beef, "NaN payload 必须逐位保留");
        }
        other => panic!("常量池形状必须保持不变，实际 {other:?}"),
    }
}

/// 上一个版本的产物必须给出明确诊断：同时点名实际版本与期望版本，
/// 不得被误解析，也不得落到模糊错误。
///
/// 上一个版本由当前 `Odin_VERSION` 推导，诊断文案也必须按同一推导值断言：
/// 写死历史阶段数值会在格式 revision 递增后变成过期断言。
#[test]
fn previous_version_artifact_reports_found_and_expected() {
    let previous = Odin_VERSION.checked_sub(1).expect("格式 revision 必须存在上一个版本");
    let mut data = module_with_constants(Vec::new());
    data.version = previous;
    let error = decode_module(&encode_module(&data)).expect_err("上一版本产物必须被拒绝");
    assert_eq!(error, OdinDecodeError::InvalidVersion(previous), "必须落到结构化版本错误，不能是别的错误");
    let text = error.to_string();
    assert!(text.contains(&format!("found {previous}")), "诊断必须点名实际版本 {previous}，实际：{text}");
    assert!(text.contains(&format!("expected {Odin_VERSION}")), "诊断必须点名期望版本 {Odin_VERSION}，实际：{text}");
}

/// 旧产物（版本 0/1）与未来产物必须结构化拒绝：常量种类编号随版本变化，兼容读取会静默错解。
#[test]
fn foreign_format_versions_are_rejected_structurally() {
    for version in [0u32, 1, Odin_VERSION + 1] {
        let mut data = module_with_constants(vec![OdinConstant::Integer64(i64::MAX)]);
        data.version = version;
        match decode_module(&encode_module(&data)) {
            Err(OdinDecodeError::InvalidVersion(rejected)) => assert_eq!(rejected, version),
            other => panic!("版本 {version} 必须以 `InvalidVersion` 拒绝，实际 {other:?}"),
        }
    }
}
