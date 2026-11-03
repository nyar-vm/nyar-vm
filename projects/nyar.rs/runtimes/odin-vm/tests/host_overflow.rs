//! 有符号整数溢出：`(MIN, -1)` 一律结构化 `ArithmeticOverflow`，不得 panic 穿透/abort 进程。
//!
//! 覆盖两条实现路径：
//! - 宿主 `CallImport` 符号（`i64_div` / `i64_rem` / `i32_div`）；
//! - i32 头码（`I32DivS` / `I32RemS`）。
//!
//! 反例不捕获 panic：直接断言返回 `Err(ArithmeticOverflow)`——若实现仍 panic，测试会以
//! panic 失败而不是被吞掉。

use odin_bytecode::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinModuleData, emit_imm1, emit_plain, encode_module,
};
use odin_vm::{
    ObjectHeap, Value,
    error::OdinRuntimeError,
    host::{HostOp, execute_host_op},
    module::LoadedModule,
    vm::OdinVm,
};

fn run(op: HostOp, args: &[Value]) -> Result<Value, OdinRuntimeError> {
    let mut heap = ObjectHeap::new();
    execute_host_op(op, args, &mut heap)
}

fn assert_overflow(result: Result<Value, OdinRuntimeError>, expected: &'static str) {
    match result {
        Ok(value) => panic!("期望 ArithmeticOverflow({expected})，实际成功返回 {value:?}"),
        Err(OdinRuntimeError::ArithmeticOverflow { operation }) => assert_eq!(operation, expected),
        Err(other) => panic!("期望 ArithmeticOverflow({expected})，实际 {other:?}"),
    }
}

/// `main() -> (头码结果) { lhs op rhs }`，走真实头码路径。
fn header_module(code_op: OdinHeadCode, lhs: i32, rhs: i32) -> LoadedModule {
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Constant, 0);
    emit_imm1(&mut code, OdinHeadCode::Constant, 1);
    emit_plain(&mut code, code_op);
    emit_plain(&mut code, OdinHeadCode::Return);
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "host_overflow".to_string(),
        constants: vec![OdinConstant::Integer32(lhs), OdinConstant::Integer32(rhs)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: Vec::new(),
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
fn i64_div_min_by_minus_one_is_arithmetic_overflow() {
    assert_overflow(run(HostOp::I64Div, &[Value::I64(i64::MIN), Value::I64(-1)]), "host i64_div");
}

#[test]
fn i64_rem_min_by_minus_one_is_arithmetic_overflow() {
    assert_overflow(run(HostOp::I64Rem, &[Value::I64(i64::MIN), Value::I64(-1)]), "host i64_rem");
}

#[test]
fn i32_div_min_by_minus_one_is_arithmetic_overflow() {
    assert_overflow(run(HostOp::I32Div, &[Value::I32(i32::MIN), Value::I32(-1)]), "host i32_div");
}

#[test]
fn i32_div_s_header_code_min_by_minus_one_is_arithmetic_overflow() {
    let module = header_module(OdinHeadCode::I32DivS, i32::MIN, -1);
    let error = OdinVm::new().run(&module, "main", Vec::new()).expect_err("(MIN, -1) 必须结构化失败");
    match error {
        OdinRuntimeError::ArithmeticOverflow { operation } => assert_eq!(operation, "i32.div_s"),
        other => panic!("期望 ArithmeticOverflow(i32.div_s)，实际 {other:?}"),
    }
}

#[test]
fn i32_rem_s_header_code_min_by_minus_one_is_arithmetic_overflow() {
    let module = header_module(OdinHeadCode::I32RemS, i32::MIN, -1);
    let error = OdinVm::new().run(&module, "main", Vec::new()).expect_err("(MIN, -1) 必须结构化失败");
    match error {
        OdinRuntimeError::ArithmeticOverflow { operation } => assert_eq!(operation, "i32.rem_s"),
        other => panic!("期望 ArithmeticOverflow(i32.rem_s)，实际 {other:?}"),
    }
}

#[test]
fn signed_division_and_remainder_stay_correct_away_from_overflow() {
    // 正例（宿主）：`MIN / 1` 不溢出；普通除法与余数保持 Rust 截断语义。
    assert_eq!(run(HostOp::I64Div, &[Value::I64(i64::MIN), Value::I64(1)]).expect("MIN / 1"), Value::I64(i64::MIN));
    assert_eq!(run(HostOp::I64Div, &[Value::I64(-7), Value::I64(2)]).expect("-7 / 2"), Value::I64(-3));
    assert_eq!(run(HostOp::I64Rem, &[Value::I64(-7), Value::I64(2)]).expect("-7 % 2"), Value::I64(-1));
    assert_eq!(run(HostOp::I32Div, &[Value::I32(i32::MIN), Value::I32(1)]).expect("MIN / 1"), Value::I32(i32::MIN));

    // 正例（头码）：`MIN / 1`、`-7 % 3` 均正常。
    let module = header_module(OdinHeadCode::I32DivS, i32::MIN, 1);
    assert_eq!(OdinVm::new().run(&module, "main", Vec::new()).expect("MIN / 1 头码"), Value::I32(i32::MIN));
    let module = header_module(OdinHeadCode::I32RemS, -7, 3);
    assert_eq!(OdinVm::new().run(&module, "main", Vec::new()).expect("-7 % 3 头码"), Value::I32(-1));
}

#[test]
fn overflow_does_not_preempt_division_by_zero() {
    // 零除数优先报 `DivisionByZero`，不得被溢出路径吞成 `ArithmeticOverflow`。
    match run(HostOp::I64Div, &[Value::I64(i64::MIN), Value::I64(0)]).expect_err("零除数") {
        OdinRuntimeError::DivisionByZero { operation } => assert_eq!(operation, "host i64_div"),
        other => panic!("期望 DivisionByZero，实际 {other:?}"),
    }
}
