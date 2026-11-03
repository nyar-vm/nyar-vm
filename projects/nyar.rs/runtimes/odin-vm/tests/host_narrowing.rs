//! `i64_to_i32` 宿主导入：窄化必须带范围校验。
//!
//! 合同：
//! - `i32` 表示范围内（含两端 `i32::MIN` / `i32::MAX`）精确返回 `Value::I32`；
//! - 超出范围（`i64::MAX`、`2147483648`、`-2147483649`、`i64::MIN`）结构化
//!   `ArithmeticOverflow`，绝不 `as i32` 静默回绕；
//! - 只接受 `Value::I64` 载体；缺参 / 其它载体结构化失败。
//!
//! 成功与失败都走真实 `CallImport` 路径，避免只测分派表。

use odin_bytecode::{
    Odin_VERSION, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinImport, OdinImportKind, OdinModuleData, emit_imm1, emit_imm2,
    emit_plain, encode_module,
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

fn assert_overflow(result: Result<Value, OdinRuntimeError>) {
    match result {
        Ok(value) => panic!("越界窄化必须结构化失败，实际成功返回 {value:?}"),
        Err(OdinRuntimeError::ArithmeticOverflow { operation }) => assert_eq!(operation, "host i64_to_i32"),
        Err(other) => panic!("越界窄化期望 ArithmeticOverflow，实际 {other:?}"),
    }
}

/// `main(value: i64) -> (导入返回值) { import(value) }`，走真实 `CallImport`。
///
/// 常量池没有 `Integer64` 载体，因此 `i64` 值只能由调用方以参数传入。
fn import_module() -> LoadedModule {
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::LoadArg, 0);
    emit_imm2(&mut code, OdinHeadCode::CallImport, 0, 1);
    emit_plain(&mut code, OdinHeadCode::Return);
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "host_narrowing".to_string(),
        constants: Vec::new(),
        functions: vec![OdinFunction { name: "main".to_string(), arity: 1, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: vec![OdinImport {
            kind: OdinImportKind::Function,
            module_name: "nyar.host".to_string(),
            symbol_name: "i64_to_i32".to_string(),
        }],
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    LoadedModule::from_bytes(&encode_module(&data)).expect("测试模块必须通过 verifier")
}

fn run_import(value: i64) -> Result<Value, OdinRuntimeError> {
    OdinVm::new().run(&import_module(), "main", vec![Value::I64(value)])
}

#[test]
fn i64_to_i32_is_registered() {
    assert_eq!(parse_host_op("i64_to_i32"), Some(HostOp::I64ToI32));
}

#[test]
fn i64_to_i32_narrows_values_inside_the_i32_range() {
    assert_eq!(run(HostOp::I64ToI32, &[Value::I64(0)]).expect("0 在范围内"), Value::I32(0));
    assert_eq!(run(HostOp::I64ToI32, &[Value::I64(i32::MAX as i64)]).expect("i32::MAX 在范围内"), Value::I32(i32::MAX));
    assert_eq!(run(HostOp::I64ToI32, &[Value::I64(i32::MIN as i64)]).expect("i32::MIN 在范围内"), Value::I32(i32::MIN));
}

#[test]
fn i64_to_i32_rejects_values_outside_the_i32_range() {
    for value in [i32::MAX as i64 + 1, i32::MIN as i64 - 1, i64::MAX, i64::MIN] {
        assert_overflow(run(HostOp::I64ToI32, &[Value::I64(value)]));
    }
}

#[test]
fn i64_to_i32_succeeds_and_fails_through_real_call_import() {
    assert_eq!(run_import(i32::MAX as i64).expect("i32::MAX 必须成功"), Value::I32(i32::MAX));
    assert_eq!(run_import(i32::MIN as i64).expect("i32::MIN 必须成功"), Value::I32(i32::MIN));
    assert_overflow(run_import(i64::MAX));
}

#[test]
fn i64_to_i32_rejects_non_i64_carriers_and_missing_arguments() {
    for bad in [Value::I32(7), Value::F64(1.0), Value::Null] {
        let error = run(HostOp::I64ToI32, &[bad]).expect_err("非 i64 载体必须结构化失败");
        match error {
            OdinRuntimeError::TypeMismatch { expected, .. } => assert_eq!(expected, "i64"),
            other => panic!("期望 TypeMismatch，实际 {other:?}"),
        }
    }
    let error = run(HostOp::I64ToI32, &[]).expect_err("缺参必须结构化失败");
    assert!(matches!(error, OdinRuntimeError::StackUnderflow), "实际 {error:?}");
}
