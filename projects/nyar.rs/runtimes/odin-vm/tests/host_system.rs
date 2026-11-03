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
