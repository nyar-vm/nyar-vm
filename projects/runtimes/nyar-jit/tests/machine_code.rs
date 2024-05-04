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
