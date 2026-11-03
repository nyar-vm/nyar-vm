//! 第 3 层：UTF-8 / UTF-16 / C 字符串宿主符号族。
//!
//! 索引空间合同：UTF-8 的 `length_chars` / `slice` 按 Unicode 码点；`slice` 是半开区间
//! `[start, end)`，越界结构化失败（不 clamp）。非法编码 fail-closed。

use odin_bytecode::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinImport, OdinImportKind, OdinModuleData, emit_imm1,
    emit_imm2, emit_plain, encode_module,
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

fn run_ok(op: HostOp, args: &[Value]) -> Value {
    run(op, args).unwrap_or_else(|error| panic!("{op:?} 必须成功，实际 {error:?}"))
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
        name: "host_text".to_string(),
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

fn text(value: &str) -> Value {
    Value::String(value.to_string())
}

// ---- 共享语料 ----
const EMPTY: &str = "";
const ASCII: &str = "abc";
const BMP_CHINESE: &str = "中文";
const ASTRAL: &str = "😀";
const COMBINING: &str = "a\u{0301}";
const LONG_ASCII: &str = "hello world";

#[test]
fn length_bytes_counts_bytes_not_code_points() {
    for (sample, bytes) in [(EMPTY, 0), (ASCII, 3), (BMP_CHINESE, 6), (ASTRAL, 4), (COMBINING, 3), (LONG_ASCII, 11)] {
        assert_eq!(run_ok(HostOp::Utf8LengthBytes, &[text(sample)]), Value::I32(bytes), "length_bytes({sample:?})");
    }
}

#[test]
fn length_chars_counts_code_points_including_astral_and_combining() {
    for (sample, chars) in [(EMPTY, 0), (ASCII, 3), (BMP_CHINESE, 2), (ASTRAL, 1), (COMBINING, 2), (LONG_ASCII, 11)] {
        assert_eq!(run_ok(HostOp::Utf8LengthChars, &[text(sample)]), Value::I32(chars), "length_chars({sample:?})");
    }
}

#[test]
fn slice_uses_half_open_code_point_indices() {
    // "a😀b"：3 个码点；emoji 占 4 字节。
    let mixed = "a😀b";
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(0), Value::I32(1)]), text("a"));
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(1), Value::I32(2)]), text("😀"), "astral 是 1 个码点");
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(2), Value::I32(3)]), text("b"));
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(0), Value::I32(3)]), text(mixed));
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(3), Value::I32(3)]), text(""), "末尾空区间合法");
    // 空串上的 [0, 0) 必须成功。
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(EMPTY), Value::I32(0), Value::I32(0)]), text(""));
    // 组合附加符是两个码点，不能被当作一个字。
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(COMBINING), Value::I32(0), Value::I32(1)]), text("a"));
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(COMBINING), Value::I32(1), Value::I32(2)]), text("\u{0301}"));
    // BMP 中文按码点切分。
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(BMP_CHINESE), Value::I32(1), Value::I32(2)]), text("文"));
}

#[test]
fn slice_rejects_out_of_bounds_without_clamping() {
    for (start, end) in [(2, 1), (0, 4), (4, 4), (-1, 1), (0, -1)] {
        let error = run(HostOp::Utf8Slice, &[text(ASCII), Value::I32(start), Value::I32(end)]).expect_err("越界必须失败");
        match error {
            OdinRuntimeError::IndexOutOfBounds { operation, start: actual_start, end: actual_end, length } => {
                assert_eq!(operation, "utf8.slice");
                assert_eq!((actual_start, actual_end, length), (start as i64, end as i64, 3));
            }
            other => panic!("期望 IndexOutOfBounds，实际 {other:?}"),
        }
    }
}

#[test]
fn length_and_slice_symbols_reject_wrong_carriers_and_short_arguments() {
    for op in [HostOp::Utf8LengthBytes, HostOp::Utf8LengthChars] {
        let error = run(op, &[Value::I32(3)]).expect_err("非字符串载体必须失败");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "{op:?} 实际 {error:?}");
        let error = run(op, &[]).expect_err("缺参必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
    }
    let error = run(HostOp::Utf8Slice, &[text(ASCII), Value::I32(0)]).expect_err("缺 end 必须失败");
    assert!(matches!(error, OdinRuntimeError::StackUnderflow));
    let error = run(HostOp::Utf8Slice, &[text(ASCII)]).expect_err("缺 start/end 必须失败");
    assert!(matches!(error, OdinRuntimeError::StackUnderflow));
    let error = run(HostOp::Utf8Slice, &[Value::Null, Value::I32(0), Value::I32(1)]).expect_err("非字符串载体必须失败");
    assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }));
}

// ---- 真实 `CallImport` 端到端 ----

#[test]
fn length_bytes_executes_through_call_import() {
    assert_eq!(run_import("utf8.length_bytes", vec![OdinConstant::String(ASTRAL.to_string())], 1), Value::I32(4));
}

#[test]
fn length_chars_executes_through_call_import() {
    assert_eq!(run_import("utf8.length_chars", vec![OdinConstant::String(ASTRAL.to_string())], 1), Value::I32(1));
    assert_eq!(run_import("utf8.length_chars", vec![OdinConstant::String(COMBINING.to_string())], 1), Value::I32(2), "组合附加符按码点计数");
}

#[test]
fn slice_executes_through_call_import() {
    assert_eq!(
        run_import("utf8.slice", vec![OdinConstant::String("a😀b".to_string()), OdinConstant::Integer32(1), OdinConstant::Integer32(2)], 3),
        Value::String("😀".to_string())
    );
}

#[test]
fn slice_out_of_bounds_fails_through_call_import() {
    let error =
        run_import_err("utf8.slice", vec![OdinConstant::String(ASCII.to_string()), OdinConstant::Integer32(0), OdinConstant::Integer32(9)], 3);
    match error {
        OdinRuntimeError::IndexOutOfBounds { operation, start, end, length } => {
            assert_eq!((operation, start, end, length), ("utf8.slice", 0, 9, 3));
        }
        other => panic!("期望 IndexOutOfBounds，实际 {other:?}"),
    }
}
