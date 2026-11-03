//! 无符号整数宿主符号：`i64_div_u` / `i64_rem_u` / `i64_shr_u` / `i32_div_u` / `i32_rem_u`。
//!
//! 这些符号此前未登记，任何 `CallImport` 到它们都会在模块加载期失败。测试覆盖
//! 最高位为 1 的操作数（有符号/无符号唯一分歧区间）与零除数，并走一次真实
//! `CallImport` 端到端路径。

use odin_bytecode::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinImport, OdinImportKind, OdinModuleData, emit_imm1,
    emit_imm2, emit_plain, encode_module,
};
use odin_vm::{
    ObjectHeap, Value,
    error::OdinRuntimeError,
    host::{HostOp, execute_host_op, parse_host_op},
    module::LoadedModule,
    vm::OdinVm,
};

fn run(op: HostOp, args: &[Value]) -> Result<Value, OdinRuntimeError> {
    let mut heap = ObjectHeap::new();
    execute_host_op(op, args, &mut heap)
}

/// 构造 `main() -> i64 { import(arg0, arg1) }`，走真实 `CallImport`。
fn import_module(symbol: &str, lhs: i32, rhs: i32) -> LoadedModule {
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Constant, 0);
    emit_imm1(&mut code, OdinHeadCode::Constant, 1);
    emit_imm2(&mut code, OdinHeadCode::CallImport, 0, 2);
    emit_plain(&mut code, OdinHeadCode::Return);
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "host_unsigned".to_string(),
        constants: vec![OdinConstant::Integer32(lhs), OdinConstant::Integer32(rhs)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: vec![OdinImport { kind: OdinImportKind::Function, module_name: "nyar.host".to_string(), symbol_name: symbol.to_string() }],
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    LoadedModule::from_bytes(&encode_module(&data)).expect("测试模块必须通过 verifier")
}

#[test]
fn unsigned_symbols_are_registered() {
    assert_eq!(parse_host_op("i64_div_u"), Some(HostOp::I64DivU));
    assert_eq!(parse_host_op("i64_rem_u"), Some(HostOp::I64RemU));
    assert_eq!(parse_host_op("i64_shr_u"), Some(HostOp::I64ShrU));
    assert_eq!(parse_host_op("i32_div_u"), Some(HostOp::I32DivU));
    assert_eq!(parse_host_op("i32_rem_u"), Some(HostOp::I32RemU));
}

#[test]
fn i64_div_u_treats_i64_min_as_positive_power_of_two() {
    // i64::MIN 的无符号位型是 2^63；除以 2 得 2^62，有符号除法会得到 -2^62。
    let min = i64::MIN;
    assert_eq!(run(HostOp::I64DivU, &[Value::I64(min), Value::I64(2)]).expect("除 2"), Value::I64(4_611_686_018_427_387_904));
    // u64::MAX / 1 = u64::MAX，回写为 -1。
    assert_eq!(run(HostOp::I64DivU, &[Value::I64(-1), Value::I64(1)]).expect("除 1"), Value::I64(-1));
    // 有符号同一输入会给出不同结果，作为分歧对照。
    assert_eq!(run(HostOp::I64Div, &[Value::I64(min), Value::I64(2)]).expect("有符号除"), Value::I64(-4_611_686_018_427_387_904));
}

#[test]
fn i64_rem_u_uses_unsigned_modulus() {
    // 2^63 mod 3 = 2；有符号 i64::MIN % 3 = -2。
    assert_eq!(run(HostOp::I64RemU, &[Value::I64(i64::MIN), Value::I64(3)]).expect("取余"), Value::I64(2));
    assert_eq!(run(HostOp::I64Rem, &[Value::I64(i64::MIN), Value::I64(3)]).expect("有符号取余"), Value::I64(-2));
    // (2^64 - 1) mod 3 = 0，有符号 -1 % 3 = -1。
    assert_eq!(run(HostOp::I64RemU, &[Value::I64(-1), Value::I64(3)]).expect("取余"), Value::I64(0));
}

#[test]
fn i64_shr_u_fills_with_zero_not_sign_bit() {
    // 0x8000_0000_0000_0000 >> 63 = 1。
    assert_eq!(run(HostOp::I64ShrU, &[Value::I64(i64::MIN), Value::I64(63)]).expect("右移"), Value::I64(1));
    // 0xFFFF_FFFF_FFFF_FFF8 >> 1 = 0x7FFF_FFFF_FFFF_FFFC。
    assert_eq!(run(HostOp::I64ShrU, &[Value::I64(-8), Value::I64(1)]).expect("右移"), Value::I64(9_223_372_036_854_775_804));
    // 位移量按 &63 截断：shift 64 等价 0。
    assert_eq!(run(HostOp::I64ShrU, &[Value::I64(-1), Value::I64(64)]).expect("右移"), Value::I64(-1));
}

#[test]
fn i32_unsigned_ops_split_high_bit() {
    // 0xFFFF_FFFF / 2 = 0x7FFF_FFFF = 2147483647；有符号 -1 / 2 = 0。
    assert_eq!(run(HostOp::I32DivU, &[Value::I32(-1), Value::I32(2)]).expect("除"), Value::I32(2_147_483_647));
    assert_eq!(run(HostOp::I32Div, &[Value::I32(-1), Value::I32(2)]).expect("有符号除"), Value::I32(0));
    // 0xFFFF_FFFE % 3 = 2；有符号 -2 % 3 = -2。
    assert_eq!(run(HostOp::I32RemU, &[Value::I32(-2), Value::I32(3)]).expect("取余"), Value::I32(2));
}

#[test]
fn unsigned_ops_reject_zero_divisor() {
    for (op, operation) in [(HostOp::I64DivU, "host i64_div_u"), (HostOp::I64RemU, "host i64_rem_u")] {
        let error = run(op, &[Value::I64(i64::MIN), Value::I64(0)]).expect_err("零除数必须失败");
        match error {
            OdinRuntimeError::DivisionByZero { operation: actual } => assert_eq!(actual, operation),
            other => panic!("期望 DivisionByZero，实际 {other:?}"),
        }
    }
    for (op, operation) in [(HostOp::I32DivU, "host i32_div_u"), (HostOp::I32RemU, "host i32_rem_u")] {
        let error = run(op, &[Value::I32(-1), Value::I32(0)]).expect_err("零除数必须失败");
        match error {
            OdinRuntimeError::DivisionByZero { operation: actual } => assert_eq!(actual, operation),
            other => panic!("期望 DivisionByZero，实际 {other:?}"),
        }
    }
}

#[test]
fn unsigned_ops_reject_non_integer_carriers() {
    for op in [HostOp::I64DivU, HostOp::I64RemU, HostOp::I64ShrU, HostOp::I32DivU, HostOp::I32RemU] {
        let error = run(op, &[Value::String("12".to_string()), Value::String("3".to_string())]).expect_err("字符串不得隐式转整数");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "{op:?} 必须报 TypeMismatch，实际 {error:?}");
    }
}

#[test]
fn i64_shr_u_executes_through_call_import() {
    // (1 << 63) >> 63 = 1，全程走真实导入：常量 → CallImport → Return。
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Constant, 0); // 1
    emit_imm1(&mut code, OdinHeadCode::Constant, 1); // 63
    emit_imm2(&mut code, OdinHeadCode::CallImport, 0, 2); // i64_shl
    emit_imm1(&mut code, OdinHeadCode::Constant, 2); // 63
    emit_imm2(&mut code, OdinHeadCode::CallImport, 1, 2); // i64_shr_u
    emit_plain(&mut code, OdinHeadCode::Return);
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "host_unsigned_e2e".to_string(),
        constants: vec![OdinConstant::Integer32(1), OdinConstant::Integer32(63), OdinConstant::Integer32(63)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: vec![
            OdinImport { kind: OdinImportKind::Function, module_name: "nyar.host".to_string(), symbol_name: "i64_shl".to_string() },
            OdinImport { kind: OdinImportKind::Function, module_name: "nyar.host".to_string(), symbol_name: "i64_shr_u".to_string() },
        ],
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    let module = LoadedModule::from_bytes(&encode_module(&data)).expect("模块必须加载成功");
    let value = OdinVm::new().run(&module, "main", Vec::new()).expect("端到端执行");
    assert_eq!(value, Value::I64(1));
}

#[test]
fn i64_div_u_executes_through_call_import() {
    // 0xFFFF_FFFF_FFFF_FFFE / 2 = 0x7FFF_FFFF_FFFF_FFFF（有符号会得 -1）。
    let module = import_module("i64_div_u", -2, 2);
    let value = OdinVm::new().run(&module, "main", Vec::new()).expect("端到端执行");
    assert_eq!(value, Value::I64(9_223_372_036_854_775_807));
}
