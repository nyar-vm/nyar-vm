use nyar_jit::*;

#[test]
fn roundtrip_ret_local() {
    let blob = encode_ret_local(3);
    assert_eq!(decode_scalar_program(&blob).unwrap(), ScalarProgram::RetLocal { slot: 3 });
}

#[test]
fn roundtrip_ret_i32_binops() {
    for binop in [I32Binop::Add, I32Binop::Sub, I32Binop::Mul, I32Binop::DivS, I32Binop::RemS] {
        let blob = encode_ret_i32_binop_locals(binop, 0, 1);
        assert_eq!(decode_scalar_program(&blob).unwrap(), ScalarProgram::RetI32BinopLocals { binop, a: 0, b: 1 });
    }
}

#[test]
fn roundtrip_ret_i32_bitwise_binops() {
    for (binop, byte) in
        [(I32Binop::And, 5u8), (I32Binop::Or, 6), (I32Binop::Xor, 7), (I32Binop::Shl, 8), (I32Binop::ShrS, 9), (I32Binop::ShrU, 10)]
    {
        assert_eq!(binop.to_u8(), byte, "{binop:?} 的编码槽位必须稳定");
        assert_eq!(I32Binop::from_u8(byte), Some(binop));
        assert!(binop.is_bitwise());
        let blob = encode_ret_i32_binop_locals(binop, 0, 1);
        assert_eq!(blob[4], 0x0F, "位运算必须走专用 opcode 0x0F 而不是算术槽位");
        assert_eq!(decode_scalar_program(&blob).unwrap(), ScalarProgram::RetI32BinopLocals { binop, a: 0, b: 1 });
    }
    assert_eq!(I32Binop::from_u8(11), None, "编码槽位之后的值必须拒绝解码");
    for binop in [I32Binop::Add, I32Binop::Sub, I32Binop::Mul, I32Binop::DivS, I32Binop::RemS] {
        assert!(!binop.is_bitwise(), "{binop:?} 已有专用算术 opcode，不得冒充位运算载荷");
    }
}

#[test]
fn rejects_bitwise_locals_blob_with_arithmetic_binop_byte() {
    // `RET_I32_BITWISE_LOCALS` 只接受位运算/移位载荷；算术 binop 字节 fail-closed。
    let mut blob = encode_ret_i32_binop_locals(I32Binop::And, 0, 1);
    blob[5] = I32Binop::Add.to_u8();
    assert!(decode_scalar_program(&blob).is_err());
    blob[5] = 0xFF;
    assert!(decode_scalar_program(&blob).is_err());
}

#[test]
fn roundtrip_ret_const_i32() {
    let blob = encode_ret_const_i32(-42);
    assert_eq!(decode_scalar_program(&blob).unwrap(), ScalarProgram::RetConstI32 { value: -42 });
}

#[test]
fn roundtrip_ret_i32_cmp_and_select() {
    let blob = encode_ret_i32_cmp_locals(I32Cmp::LtS, 0, 1);
    assert_eq!(decode_scalar_program(&blob).unwrap(), ScalarProgram::RetI32CmpLocals { cmp: I32Cmp::LtS, a: 0, b: 1 });
    let blob = encode_ret_i32_select_cmp_locals(I32Cmp::Eq, 0, 1, 2, 3);
    assert_eq!(
        decode_scalar_program(&blob).unwrap(),
        ScalarProgram::RetI32SelectCmpLocals { cmp: I32Cmp::Eq, a: 0, b: 1, then_slot: 2, else_slot: 3 }
    );
}

#[test]
fn roundtrip_ret_i32_select_cmp_consts() {
    let blob = encode_ret_i32_select_cmp_consts(I32Cmp::Eq, 0, 1, 7, 9);
    assert_eq!(
        decode_scalar_program(&blob).unwrap(),
        ScalarProgram::RetI32SelectCmpConsts { cmp: I32Cmp::Eq, a: 0, b: 1, then_imm: 7, else_imm: 9 }
    );
}

#[test]
fn roundtrip_ret_i32_imm_local() {
    let blob = encode_ret_i32_binop_imm_local(I32Binop::Add, 10, 0, true);
    assert_eq!(
        decode_scalar_program(&blob).unwrap(),
        ScalarProgram::RetI32BinopImmLocal { binop: I32Binop::Add, imm: 10, local: 0, imm_on_left: true }
    );
    let blob = encode_ret_i32_cmp_imm_local(I32Cmp::LtS, 5, 1, false);
    assert_eq!(
        decode_scalar_program(&blob).unwrap(),
        ScalarProgram::RetI32CmpImmLocal { cmp: I32Cmp::LtS, imm: 5, local: 1, imm_on_left: false }
    );
}

#[test]
fn roundtrip_ret_i32_select_cmp_mixed_and_void() {
    let blob = encode_ret_i32_select_cmp_mixed(I32Cmp::Eq, 0, 1, SelectArm::Imm(7), SelectArm::Local(2));
    assert_eq!(
        decode_scalar_program(&blob).unwrap(),
        ScalarProgram::RetI32SelectCmpMixed { cmp: I32Cmp::Eq, a: 0, b: 1, then_arm: SelectArm::Imm(7), else_arm: SelectArm::Local(2) }
    );
    let blob = encode_ret_i32_select_cmp_mixed(I32Cmp::Ne, 0, 1, SelectArm::Local(3), SelectArm::Imm(9));
    assert_eq!(
        decode_scalar_program(&blob).unwrap(),
        ScalarProgram::RetI32SelectCmpMixed { cmp: I32Cmp::Ne, a: 0, b: 1, then_arm: SelectArm::Local(3), else_arm: SelectArm::Imm(9) }
    );
    assert_eq!(decode_scalar_program(&encode_ret_void()).unwrap(), ScalarProgram::RetVoid);
}

#[test]
fn roundtrip_unsigned_i32_cmp_encoding_and_semantics() {
    for (cmp, byte) in [(I32Cmp::LtU, 6u8), (I32Cmp::LeU, 7u8), (I32Cmp::GtU, 8u8), (I32Cmp::GeU, 9u8)] {
        assert_eq!(cmp.to_u8(), byte, "{cmp:?} 的编码槽位必须稳定");
        assert_eq!(I32Cmp::from_u8(byte), Some(cmp));
        let blob = encode_ret_i32_cmp_locals(cmp, 0, 1);
        assert_eq!(decode_scalar_program(&blob).unwrap(), ScalarProgram::RetI32CmpLocals { cmp, a: 0, b: 1 });
    }
    assert_eq!(I32Cmp::from_u8(10), None, "编码槽位之后的值必须拒绝解码");

    // 无符号语义：`-1` 的位型是 `0xFFFFFFFF`，对 `1` 必须判大不判小。
    assert!(!I32Cmp::LtU.eval(-1, 1));
    assert!(!I32Cmp::LeU.eval(-1, 1));
    assert!(I32Cmp::GtU.eval(-1, 1));
    assert!(I32Cmp::GeU.eval(-1, 1));
    assert!(I32Cmp::LtU.eval(3, 5));
    assert!(!I32Cmp::GtU.eval(3, 5));
}
