//! 第 4 层：console / time / random / math 宿主符号族。

use odin_bytecode::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinImport, OdinImportKind, OdinModuleData, emit_imm1,
    emit_imm2, emit_plain, encode_module,
};
use odin_vm::{
    ObjectHeap, Value,
    error::OdinRuntimeError,
    host::{HostOp, execute_host_op},
    module::LoadedModule,
    output_runtime::capture,
    vm::OdinVm,
};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn run(op: HostOp, args: &[Value]) -> Result<Value, OdinRuntimeError> {
    let mut heap = ObjectHeap::new();
    execute_host_op(op, args, &mut heap)
}

fn run_ok(op: HostOp, args: &[Value]) -> Value {
    run(op, args).unwrap_or_else(|error| panic!("{op:?} 必须成功，实际 {error:?}"))
}

fn text(value: &str) -> Value {
    Value::String(value.to_string())
}

/// 构造 `main() -> (导入返回值) { import(constant0, …, constant{N-1}) }`，走真实 `CallImport`。
fn import_module(symbol: &str, constants: Vec<OdinConstant>, argc: i32) -> LoadedModule {
    let mut code = Vec::new();
    for index in 0..argc {
        emit_imm1(&mut code, OdinHeadCode::Constant, index);
    }
    emit_imm2(&mut code, OdinHeadCode::CallImport, 0, argc);
    emit_plain(&mut code, OdinHeadCode::Return);
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "host_system".to_string(),
        constants,
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

fn run_import(symbol: &str, constants: Vec<OdinConstant>, argc: i32) -> Value {
    let module = import_module(symbol, constants, argc);
    OdinVm::new().run(&module, "main", Vec::new()).expect("端到端执行必须成功")
}

fn run_import_err(symbol: &str, constants: Vec<OdinConstant>, argc: i32) -> OdinRuntimeError {
    let module = import_module(symbol, constants, argc);
    OdinVm::new().run(&module, "main", Vec::new()).expect_err("端到端执行必须结构化失败")
}

#[test]
fn println_outputs_i32_decimal_and_returns_argument() {
    let (value, lines) = capture(|| run_ok(HostOp::Println, &[Value::I32(-42)]));
    assert_eq!(value, Value::I32(-42));
    assert_eq!(lines, vec!["-42".to_string()]);
}

#[test]
fn println_outputs_utf8_verbatim_and_returns_argument() {
    let (value, lines) = capture(|| run_ok(HostOp::Println, &[text("中文😀")]));
    assert_eq!(value, text("中文😀"));
    assert_eq!(lines, vec!["中文😀".to_string()]);
}

#[test]
fn println_executes_through_call_import_with_captured_output() {
    let (value, lines) = capture(|| run_import("println", vec![OdinConstant::Integer32(7)], 1));
    assert_eq!(value, Value::I32(7));
    assert_eq!(lines, vec!["7".to_string()]);

    let (value, lines) = capture(|| run_import("println", vec![OdinConstant::String("中文😀".to_string())], 1));
    assert_eq!(value, text("中文😀"));
    assert_eq!(lines, vec!["中文😀".to_string()]);
}

#[test]
fn println_rejects_non_declared_carriers_and_short_arguments() {
    for carrier in [Value::I64(1), Value::F64(1.0), Value::Null, Value::Bool(true)] {
        let error = run(HostOp::Println, &[carrier.clone()]).expect_err("非声明载体必须失败");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "载体 {carrier:?} 实际 {error:?}");
    }
    let error = run(HostOp::Println, &[]).expect_err("缺参必须失败");
    assert!(matches!(error, OdinRuntimeError::StackUnderflow));
    let error = run_import_err("println", Vec::new(), 0);
    assert!(matches!(error, OdinRuntimeError::StackUnderflow));
}

#[test]
fn print_keeps_variadic_tab_separated_behavior() {
    let (value, lines) = capture(|| run_ok(HostOp::Print, &[Value::I32(1), text("x"), Value::Bool(true)]));
    assert_eq!(value, Value::Bool(true), "print 仍返回最后一个参数");
    assert_eq!(lines, vec!["1\tx\ttrue".to_string()]);
}

/// 墙钟毫秒只做区间断言：不写死具体值，避免校时 / 调度造成 flaky。
#[test]
fn get_time_returns_unix_epoch_millis_within_call_interval() {
    let before = SystemTime::now().duration_since(UNIX_EPOCH).expect("测试机时钟早于纪元").as_millis() as i64;
    let value = run_ok(HostOp::GetTime, &[]);
    let after = SystemTime::now().duration_since(UNIX_EPOCH).expect("测试机时钟早于纪元").as_millis() as i64;
    let Value::I64(millis) = value
    else {
        panic!("get_time 必须返回 i64，实际 {value:?}");
    };
    assert!((before..=after).contains(&millis), "get_time 落在调用区间内：before {before} value {millis} after {after}");
}

/// 单调毫秒连续取值必须非递减；不写死差值。
#[test]
fn monotonic_ms_never_goes_backwards() {
    let mut previous = match run_ok(HostOp::MonotonicMs, &[]) {
        Value::I64(value) => value,
        other => panic!("monotonic_ms 必须返回 i64，实际 {other:?}"),
    };
    for _ in 0..8 {
        let current = match run_ok(HostOp::MonotonicMs, &[]) {
            Value::I64(value) => value,
            other => panic!("monotonic_ms 必须返回 i64，实际 {other:?}"),
        };
        assert!(current >= previous, "单调时钟回退：previous {previous} current {current}");
        previous = current;
    }
}

#[test]
fn get_time_and_monotonic_ms_execute_through_call_import() {
    let value = run_import("get_time", Vec::new(), 0);
    let Value::I64(millis) = value
    else {
        panic!("CallImport get_time 必须返回 i64，实际 {value:?}");
    };
    assert!(millis > 0, "Unix 纪元毫秒必须为正，实际 {millis}");

    let value = run_import("monotonic_ms", Vec::new(), 0);
    let Value::I64(millis) = value
    else {
        panic!("CallImport monotonic_ms 必须返回 i64，实际 {value:?}");
    };
    assert!(millis >= 0, "单调毫秒不得为负，实际 {millis}");
}

#[test]
fn sleep_ms_returns_declared_i32_and_rejects_negative_values() {
    for ms in [0, 1, 50] {
        let started = Instant::now();
        let value = run_ok(HostOp::SleepMs, &[Value::I32(ms)]);
        assert_eq!(value, Value::I32(ms), "sleep_ms 原样返回入参");
        assert!(started.elapsed().as_millis() < 5_000, "sleep_ms({ms}) 不应拖到秒级");
    }

    for ms in [-1, i32::MIN] {
        let error = run(HostOp::SleepMs, &[Value::I32(ms)]).expect_err("负值必须结构化失败");
        assert!(
            matches!(error, OdinRuntimeError::NegativeArgument { operation, value } if operation == "sleep_ms" && value == ms as i64),
            "sleep_ms({ms}) 实际 {error:?}"
        );
    }

    let error = run_import_err("sleep_ms", vec![OdinConstant::Integer32(-3)], 1);
    assert!(matches!(error, OdinRuntimeError::NegativeArgument { operation: "sleep_ms", value: -3 }), "CallImport sleep_ms(-3) 实际 {error:?}");

    let error = run(HostOp::SleepMs, &[text("5")]).expect_err("非数值载体必须失败");
    assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "实际 {error:?}");
}
