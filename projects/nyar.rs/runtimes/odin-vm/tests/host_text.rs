//! 第 3 层：UTF-8 / UTF-16 / C 字符串宿主符号族。
//!
//! 索引空间合同：UTF-8 的 `length_chars` / `slice` 按 Unicode 码点；`slice` 是半开区间
//! `[start, end)`，越界结构化失败（不 clamp）。非法编码 fail-closed。

use acorn_odin::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinImport, OdinImportKind, OdinModuleData, emit_imm1,
    emit_imm2, emit_plain, encode_module,
};
use odin_vm::{
    ObjectHeap, ObjectPayload, Value, array_runtime,
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

/// 构造多步导入链：常量按序入栈，随后按 `steps`（导入下标, 参数个数）依次 `CallImport`。
fn chain_module(constants: Vec<OdinConstant>, imports: Vec<&str>, steps: &[(i32, i32)]) -> LoadedModule {
    let mut code = Vec::new();
    for index in 0..constants.len() as i32 {
        emit_imm1(&mut code, OdinHeadCode::Constant, index);
    }
    for (import_index, argc) in steps {
        emit_imm2(&mut code, OdinHeadCode::CallImport, *import_index, *argc);
    }
    emit_plain(&mut code, OdinHeadCode::Return);
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "host_text_chain".to_string(),
        constants,
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: imports
            .into_iter()
            .map(|symbol| OdinImport { kind: OdinImportKind::Function, module_name: "nyar.host".to_string(), symbol_name: symbol.to_string() })
            .collect(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    LoadedModule::from_bytes(&encode_module(&data)).expect("测试模块必须通过 verifier")
}

fn run_import_chain(constants: Vec<OdinConstant>, imports: Vec<&str>, steps: &[(i32, i32)]) -> Value {
    let module = chain_module(constants, imports, steps);
    OdinVm::new().run(&module, "main", Vec::new()).expect("端到端执行必须成功")
}

fn text(value: &str) -> Value {
    Value::String(value.to_string())
}

fn i32_of(value: Value) -> i32 {
    match value {
        Value::I32(number) => number,
        other => panic!("期望 i32，实际 {other:?}"),
    }
}

fn string_of(value: Value) -> String {
    match value {
        Value::String(text) => text,
        other => panic!("期望字符串，实际 {other:?}"),
    }
}

/// 把码元序列包成宿主的 UTF-16 固定数组载体。
fn utf16_array(heap: &mut ObjectHeap, units: &[u16]) -> Value {
    let elements = units.iter().map(|unit| Value::I32(*unit as i32)).collect::<Vec<_>>();
    array_runtime::build_fixed_array(heap, 0, &elements)
}

/// 从宿主 UTF-16 固定数组载体读回码元序列。
fn utf16_units(heap: &ObjectHeap, value: &Value) -> Vec<u16> {
    let Value::Object(object_id) = value
    else {
        panic!("期望 UTF-16 数组载体，实际 {}", value.type_name());
    };
    let Some(ObjectPayload::LayoutObject { slots, .. }) = heap.get(*object_id)
    else {
        panic!("期望布局数组对象，实际 object id {object_id}");
    };
    slots
        .iter()
        .map(|slot| match slot {
            Value::I32(unit) => *unit as u16,
            other => panic!("期望 i32 码元槽，实际 {}", other.type_name()),
        })
        .collect()
}

/// 用给定码元构造数组后执行 UTF-16 宿主符号。
fn run_utf16(op: HostOp, units: &[u16]) -> Result<Value, OdinRuntimeError> {
    let mut heap = ObjectHeap::new();
    let array = utf16_array(&mut heap, units);
    execute_host_op(op, &[array], &mut heap)
}

/// 把字节序列包成宿主的 C 字符串固定数组载体。
fn c_str_array(heap: &mut ObjectHeap, bytes: &[u8]) -> Value {
    let elements = bytes.iter().map(|byte| Value::I32(*byte as i32)).collect::<Vec<_>>();
    array_runtime::build_fixed_array(heap, 0, &elements)
}

/// 从宿主 C 字符串固定数组载体读回字节序列。
fn c_str_bytes(heap: &ObjectHeap, value: &Value) -> Vec<u8> {
    let Value::Object(object_id) = value
    else {
        panic!("期望 C 字符串数组载体，实际 {}", value.type_name());
    };
    let Some(ObjectPayload::LayoutObject { slots, .. }) = heap.get(*object_id)
    else {
        panic!("期望布局数组对象，实际 object id {object_id}");
    };
    slots
        .iter()
        .map(|slot| match slot {
            Value::I32(byte) => *byte as u8,
            other => panic!("期望 i32 字节槽，实际 {}", other.type_name()),
        })
        .collect()
}

/// 用给定字节构造 C 字符串数组后执行宿主符号。
fn run_cstr(op: HostOp, bytes: &[u8]) -> Result<Value, OdinRuntimeError> {
    let mut heap = ObjectHeap::new();
    let array = c_str_array(&mut heap, bytes);
    execute_host_op(op, &[array], &mut heap)
}

// ---- 共享语料 ----
const EMPTY: &str = "";
const ASCII: &str = "abc";
const BMP_CHINESE: &str = "中文";
const ASTRAL: &str = "😀";
const COMBINING: &str = "a\u{0301}";
const LONG_ASCII: &str = "hello world";
/// 单个占满 4 字节的码点（U+10FFFF）。
const MAX_CODE_POINT: &str = "\u{10FFFF}";
/// 混合语料：BMP 中文 + astral + ASCII + 组合附加符。
const MIXED: &str = "中文😀a\u{0301}";

#[test]
fn length_bytes_counts_bytes_not_code_points() {
    for (sample, bytes) in
        [(EMPTY, 0), (ASCII, 3), (BMP_CHINESE, 6), (ASTRAL, 4), (COMBINING, 3), (LONG_ASCII, 11), (MAX_CODE_POINT, 4), (MIXED, 13)]
    {
        assert_eq!(run_ok(HostOp::Utf8LengthBytes, &[text(sample)]), Value::I32(bytes), "length_bytes({sample:?})");
    }
}

#[test]
fn length_chars_counts_code_points_including_astral_and_combining() {
    for (sample, chars) in
        [(EMPTY, 0), (ASCII, 3), (BMP_CHINESE, 2), (ASTRAL, 1), (COMBINING, 2), (LONG_ASCII, 11), (MAX_CODE_POINT, 1), (MIXED, 5)]
    {
        assert_eq!(run_ok(HostOp::Utf8LengthChars, &[text(sample)]), Value::I32(chars), "length_chars({sample:?})");
    }
}

/// 已废弃的 end 语义表（第 3 层首版 `slice(start, end)`，保留作为变更证据，不再参与断言）：
/// `("a😀b", 0, 1) -> "a"`、`("a😀b", 1, 2) -> "😀"`、`("a😀b", 2, 3) -> "b"`、
/// `("a😀b", 0, 3) -> "a😀b"`、`("a😀b", 3, 3) -> ""`、`("中文", 1, 2) -> "文"`。
/// 第三参数从 end 改为 count 后，同一组输入除 `(0,3)` 外语义全部不同。
#[test]
fn slice_uses_code_point_start_and_count() {
    // "a😀b"：3 个码点；emoji 占 4 字节。
    let mixed = "a😀b";
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(0), Value::I32(1)]), text("a"));
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(1), Value::I32(1)]), text("😀"), "astral 是 1 个码点");
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(2), Value::I32(1)]), text("b"));
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(1), Value::I32(2)]), text("😀b"), "count 是码点个数");
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(0), Value::I32(3)]), text(mixed));
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(mixed), Value::I32(3), Value::I32(0)]), text(""), "start == length 且 count == 0 合法");
    // 空串上的 (0, 0) 必须成功。
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(EMPTY), Value::I32(0), Value::I32(0)]), text(""));
    // 组合附加符是两个码点，不能被当作一个字。
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(COMBINING), Value::I32(0), Value::I32(1)]), text("a"));
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(COMBINING), Value::I32(1), Value::I32(1)]), text("\u{0301}"));
    // BMP 中文按码点切分。
    assert_eq!(run_ok(HostOp::Utf8Slice, &[text(BMP_CHINESE), Value::I32(1), Value::I32(1)]), text("文"));
}

#[test]
fn slice_rejects_out_of_bounds_without_clamping() {
    // ASCII 长度 3；count 语义下的越界反例。
    for (start, count) in [(4, 0), (3, 1), (0, 4), (2, 2), (-1, 1), (0, -1)] {
        let error = run(HostOp::Utf8Slice, &[text(ASCII), Value::I32(start), Value::I32(count)]).expect_err("越界必须失败");
        match error {
            OdinRuntimeError::IndexOutOfBounds { operation, start: actual_start, count: actual_count, length } => {
                assert_eq!(operation, "utf8.slice");
                assert_eq!((actual_start, actual_count, length), (start as i64, count as i64, 3));
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
    let error = run(HostOp::Utf8Slice, &[text(ASCII), Value::I32(0)]).expect_err("缺 count 必须失败");
    assert!(matches!(error, OdinRuntimeError::StackUnderflow));
    let error = run(HostOp::Utf8Slice, &[text(ASCII)]).expect_err("缺 start/count 必须失败");
    assert!(matches!(error, OdinRuntimeError::StackUnderflow));
    let error = run(HostOp::Utf8Slice, &[Value::Null, Value::I32(0), Value::I32(1)]).expect_err("非字符串载体必须失败");
    assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }));
}

#[test]
fn concat_preserves_both_sides_verbatim() {
    for (left, right, joined) in [
        (EMPTY, EMPTY, ""),
        (EMPTY, ASCII, "abc"),
        (ASCII, EMPTY, "abc"),
        (BMP_CHINESE, ASTRAL, "中文😀"),
        (ASTRAL, BMP_CHINESE, "😀中文"),
        (COMBINING, ASTRAL, "a\u{0301}😀"),
        (LONG_ASCII, BMP_CHINESE, "hello world中文"),
    ] {
        assert_eq!(run_ok(HostOp::Utf8Concat, &[text(left), text(right)]), text(joined), "concat({left:?}, {right:?})");
    }
}

#[test]
fn eq_and_ne_compare_code_point_sequences_without_normalization() {
    // 组合式 `a` + U+0301 与预组合式 U+00E9 视觉近似，但码点序列不同 —— 不做规范化。
    let decomposed = "e\u{0301}";
    let precomposed = "\u{e9}";
    assert_eq!(run_ok(HostOp::Utf8Eq, &[text(decomposed), text(precomposed)]), Value::Bool(false));
    assert_eq!(run_ok(HostOp::Utf8Ne, &[text(decomposed), text(precomposed)]), Value::Bool(true));
    for sample in [EMPTY, ASCII, BMP_CHINESE, ASTRAL, COMBINING, LONG_ASCII] {
        assert_eq!(run_ok(HostOp::Utf8Eq, &[text(sample), text(sample)]), Value::Bool(true), "eq({sample:?}, self)");
        assert_eq!(run_ok(HostOp::Utf8Ne, &[text(sample), text(sample)]), Value::Bool(false), "ne({sample:?}, self)");
    }
    assert_eq!(run_ok(HostOp::Utf8Eq, &[text(ASCII), text(BMP_CHINESE)]), Value::Bool(false));
    assert_eq!(run_ok(HostOp::Utf8Ne, &[text(ASCII), text(BMP_CHINESE)]), Value::Bool(true));
    // 码点序列相等：同一个 astral 码点在不同写法上仍是同一序列。
    assert_eq!(run_ok(HostOp::Utf8Eq, &[text(ASTRAL), text("\u{1F600}")]), Value::Bool(true));
}

#[test]
fn concat_and_comparison_reject_wrong_carriers_and_short_arguments() {
    for op in [HostOp::Utf8Concat, HostOp::Utf8Eq, HostOp::Utf8Ne] {
        let error = run(op, &[text(ASCII)]).expect_err("缺第二参必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
        let error = run(op, &[]).expect_err("缺参必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
        let error = run(op, &[text(ASCII), Value::I32(1)]).expect_err("非字符串载体必须失败");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "{op:?} 实际 {error:?}");
    }
}

// ---- 长度与 slice 的交叉断言 ----

#[test]
fn length_chars_matches_code_point_walk_via_slice() {
    let corpus = [EMPTY, ASCII, BMP_CHINESE, ASTRAL, COMBINING, LONG_ASCII, MAX_CODE_POINT, MIXED];
    for sample in corpus {
        let chars = i32_of(run_ok(HostOp::Utf8LengthChars, &[text(sample)]));
        let mut segments = 0;
        for start in 0..chars {
            let segment = string_of(run_ok(HostOp::Utf8Slice, &[text(sample), Value::I32(start), Value::I32(1)]));
            assert!(!segment.is_empty(), "slice({sample:?}, {start}, 1) 不能是空段");
            segments += 1;
        }
        assert_eq!(segments, chars, "length_chars 必须等于逐码点 slice 的段数：{sample:?}");
    }
}

#[test]
fn length_bytes_matches_whole_slice_byte_length() {
    let corpus = [EMPTY, ASCII, BMP_CHINESE, ASTRAL, COMBINING, LONG_ASCII, MAX_CODE_POINT, MIXED];
    for sample in corpus {
        let chars = i32_of(run_ok(HostOp::Utf8LengthChars, &[text(sample)]));
        let bytes = i32_of(run_ok(HostOp::Utf8LengthBytes, &[text(sample)]));
        let whole = string_of(run_ok(HostOp::Utf8Slice, &[text(sample), Value::I32(0), Value::I32(chars)]));
        assert_eq!(whole, sample, "整串 slice 必须还原原串：{sample:?}");
        assert_eq!(whole.len() as i32, bytes, "slice(0, length_chars) 的字节数必须等于 length_bytes：{sample:?}");
    }
}

// ---- UTF-16 ----

#[test]
fn utf8_to_utf16_emits_surrogate_pairs() {
    let cases: [(&str, Vec<u16>); 6] = [
        (EMPTY, vec![]),
        (ASCII, vec![0x61, 0x62, 0x63]),
        (BMP_CHINESE, vec![0x4E2D, 0x6587]),
        (ASTRAL, vec![0xD83D, 0xDE00]),
        (COMBINING, vec![0x61, 0x0301]),
        (LONG_ASCII, vec![0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x77, 0x6F, 0x72, 0x6C, 0x64]),
    ];
    let mut heap = ObjectHeap::new();
    for (sample, expected) in cases {
        let value = execute_host_op(HostOp::Utf8ToUtf16, &[text(sample)], &mut heap).unwrap();
        assert_eq!(utf16_units(&heap, &value), expected, "utf8_to_utf16({sample:?})");
    }
}

#[test]
fn utf16_length_counts_code_units_not_code_points() {
    let cases: [(Vec<u16>, i32); 5] =
        [(vec![], 0), (vec![0x61, 0x62, 0x63], 3), (vec![0x4E2D, 0x6587], 2), (vec![0xD83D, 0xDE00], 2), (vec![0x61, 0x0301], 2)];
    for (units, expected) in cases {
        assert_eq!(run_utf16(HostOp::Utf16Length, &units).unwrap(), Value::I32(expected), "utf16_length({units:?})");
    }
}

#[test]
fn utf16_to_utf8_round_trips_all_corpora() {
    let cases: [(Vec<u16>, &str); 6] = [
        (vec![], EMPTY),
        (vec![0x61, 0x62, 0x63], ASCII),
        (vec![0x4E2D, 0x6587], BMP_CHINESE),
        (vec![0xD83D, 0xDE00], ASTRAL),
        (vec![0x61, 0x0301], COMBINING),
        (vec![0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x77, 0x6F, 0x72, 0x6C, 0x64], LONG_ASCII),
    ];
    for (units, expected) in cases {
        assert_eq!(run_utf16(HostOp::Utf16ToUtf8, &units).unwrap(), text(expected), "utf16_to_utf8({units:?})");
    }
}

#[test]
fn utf16_to_utf8_rejects_lone_surrogates() {
    let cases: [Vec<u16>; 6] =
        [vec![0xD83D], vec![0xDE00], vec![0xD83D, 0x0041], vec![0x0041, 0xDE00], vec![0xD83D, 0xD83D], vec![0xDE00, 0xD83D]];
    for units in cases {
        let error = run_utf16(HostOp::Utf16ToUtf8, &units).expect_err("孤立代理必须失败");
        match error {
            OdinRuntimeError::InvalidUtf8 { operation } => assert_eq!(operation, "utf16_to_utf8", "输入 {units:?}"),
            other => panic!("期望 InvalidUtf8，实际 {other:?}（输入 {units:?}）"),
        }
    }
}

#[test]
fn utf16_symbols_reject_wrong_carriers_and_short_arguments() {
    for op in [HostOp::Utf16Length, HostOp::Utf16ToUtf8] {
        let error = run(op, &[text(ASCII)]).expect_err("字符串不是 UTF-16 载体");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "{op:?} 实际 {error:?}");
        let error = run(op, &[]).expect_err("缺参必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
    }
    let error = run(HostOp::Utf8ToUtf16, &[Value::I32(1)]).expect_err("非字符串入参必须失败");
    assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "实际 {error:?}");
    let error = run(HostOp::Utf8ToUtf16, &[]).expect_err("缺参必须失败");
    assert!(matches!(error, OdinRuntimeError::StackUnderflow));

    // 非法码元槽：超出 u16 范围 / 非整数载体。
    let mut heap = ObjectHeap::new();
    for element in [Value::I32(0x1_0000), Value::F64(1.0), Value::Null] {
        let array = array_runtime::build_fixed_array(&mut heap, 0, &[element.clone()]);
        let error = execute_host_op(HostOp::Utf16Length, &[array.clone()], &mut heap).expect_err("非法码元必须失败");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "元素 {element:?} 实际 {error:?}");
        let error = execute_host_op(HostOp::Utf16ToUtf8, &[array], &mut heap).expect_err("非法码元必须失败");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "元素 {element:?} 实际 {error:?}");
    }
}

// ---- C 字符串 ----

#[test]
fn utf8_to_c_str_appends_terminator_and_preserves_bytes() {
    let cases: [(&str, Vec<u8>); 6] = [
        (EMPTY, vec![0]),
        (ASCII, vec![0x61, 0x62, 0x63, 0]),
        (BMP_CHINESE, vec![0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87, 0]),
        (ASTRAL, vec![0xF0, 0x9F, 0x98, 0x80, 0]),
        (COMBINING, vec![0x61, 0xCC, 0x81, 0]),
        (LONG_ASCII, vec![0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x77, 0x6F, 0x72, 0x6C, 0x64, 0]),
    ];
    let mut heap = ObjectHeap::new();
    for (sample, expected) in cases {
        let value = execute_host_op(HostOp::Utf8ToCStr, &[text(sample)], &mut heap).unwrap();
        assert_eq!(c_str_bytes(&heap, &value), expected, "utf8_to_c_str({sample:?})");
    }
}

#[test]
fn utf8_to_c_str_rejects_embedded_nul() {
    for sample in ["a\0b", "\0", "\0abc"] {
        let error = run(HostOp::Utf8ToCStr, &[text(sample)]).expect_err("内嵌 NUL 必须失败");
        match error {
            OdinRuntimeError::EmbeddedNul { operation } => assert_eq!(operation, "utf8_to_c_str", "输入 {sample:?}"),
            other => panic!("期望 EmbeddedNul，实际 {other:?}"),
        }
    }
}

#[test]
fn c_str_to_utf8_reads_until_first_nul() {
    let cases: [(Vec<u8>, &str); 7] = [
        (vec![0], EMPTY),
        (vec![0x61, 0x62, 0x63, 0], ASCII),
        (vec![0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87, 0], BMP_CHINESE),
        (vec![0xF0, 0x9F, 0x98, 0x80, 0], ASTRAL),
        (vec![0x61, 0xCC, 0x81, 0], COMBINING),
        (vec![0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x77, 0x6F, 0x72, 0x6C, 0x64, 0], LONG_ASCII),
        // 首个 NUL 之后的内容不参与解码。
        (vec![0x61, 0, 0x62], "a"),
    ];
    for (bytes, expected) in cases {
        assert_eq!(run_cstr(HostOp::CStrToUtf8, &bytes).unwrap(), text(expected), "c_str_to_utf8({bytes:?})");
    }
}

#[test]
fn c_str_operations_reject_unterminated_buffers() {
    for (op, operation) in
        [(HostOp::CStrToUtf8, "c_str_to_utf8"), (HostOp::CStrToUtf16, "c_str_to_utf16"), (HostOp::CStrLength, "c_str_length")]
    {
        let error = run_cstr(op, &[0x61, 0x62]).expect_err("缺终止 NUL 必须失败");
        match error {
            OdinRuntimeError::UnterminatedCStr { operation: actual } => assert_eq!(actual, operation),
            other => panic!("{op:?} 期望 UnterminatedCStr，实际 {other:?}"),
        }
    }
    // 空缓冲区同样没有 NUL，不能当作长度为 0 的 C 字符串。
    assert!(matches!(run_cstr(HostOp::CStrLength, &[]).expect_err("空缓冲区没有 NUL"), OdinRuntimeError::UnterminatedCStr { .. }));
}

#[test]
fn c_str_to_utf8_rejects_invalid_utf8() {
    let invalid: [&[u8]; 4] = [&[0xFF, 0], &[0xC0, 0x80, 0], &[0xE4, 0xB8, 0], &[0x80, 0]];
    for bytes in invalid {
        let error = run_cstr(HostOp::CStrToUtf8, bytes).expect_err("非法 UTF-8 必须失败");
        match error {
            OdinRuntimeError::InvalidUtf8 { operation } => assert_eq!(operation, "c_str_to_utf8", "输入 {bytes:?}"),
            other => panic!("期望 InvalidUtf8，实际 {other:?}"),
        }
        let error = run_cstr(HostOp::CStrToUtf16, bytes).expect_err("非法 UTF-8 必须失败");
        match error {
            OdinRuntimeError::InvalidUtf8 { operation } => assert_eq!(operation, "c_str_to_utf16", "输入 {bytes:?}"),
            other => panic!("期望 InvalidUtf8，实际 {other:?}"),
        }
    }
}

#[test]
fn c_str_length_counts_content_bytes() {
    let cases: [(Vec<u8>, i32); 7] = [
        (vec![0], 0),
        (vec![0x61, 0x62, 0x63, 0], 3),
        (vec![0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87, 0], 6),
        (vec![0xF0, 0x9F, 0x98, 0x80, 0], 4),
        (vec![0x61, 0xCC, 0x81, 0], 3),
        (vec![0x68, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x77, 0x6F, 0x72, 0x6C, 0x64, 0], 11),
        (vec![0x61, 0, 0x62], 1),
    ];
    for (bytes, expected) in cases {
        assert_eq!(run_cstr(HostOp::CStrLength, &bytes).unwrap(), Value::I32(expected), "c_str_length({bytes:?})");
    }
}

#[test]
fn c_str_to_utf16_encodes_surrogate_pairs() {
    let cases: [(Vec<u8>, Vec<u16>); 3] = [
        (vec![0], vec![]),
        (vec![0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87, 0], vec![0x4E2D, 0x6587]),
        (vec![0xF0, 0x9F, 0x98, 0x80, 0], vec![0xD83D, 0xDE00]),
    ];
    let mut heap = ObjectHeap::new();
    for (bytes, expected) in cases {
        let array = c_str_array(&mut heap, &bytes);
        let value = execute_host_op(HostOp::CStrToUtf16, &[array], &mut heap).unwrap();
        assert_eq!(utf16_units(&heap, &value), expected, "c_str_to_utf16({bytes:?})");
    }
}

#[test]
fn utf16_to_c_str_encodes_and_terminates() {
    let cases: [(Vec<u16>, Vec<u8>); 4] = [
        (vec![], vec![0]),
        (vec![0x4E2D, 0x6587], vec![0xE4, 0xB8, 0xAD, 0xE6, 0x96, 0x87, 0]),
        (vec![0xD83D, 0xDE00], vec![0xF0, 0x9F, 0x98, 0x80, 0]),
        (vec![0x61, 0x0301], vec![0x61, 0xCC, 0x81, 0]),
    ];
    let mut heap = ObjectHeap::new();
    for (units, expected) in cases {
        let array = utf16_array(&mut heap, &units);
        let value = execute_host_op(HostOp::Utf16ToCStr, &[array], &mut heap).unwrap();
        assert_eq!(c_str_bytes(&heap, &value), expected, "utf16_to_c_str({units:?})");
    }
}

#[test]
fn utf16_to_c_str_rejects_lone_surrogates_and_embedded_nul() {
    let error = run_utf16(HostOp::Utf16ToCStr, &[0xD83D]).expect_err("孤立代理必须失败");
    match error {
        OdinRuntimeError::InvalidUtf8 { operation } => assert_eq!(operation, "utf16_to_c_str"),
        other => panic!("期望 InvalidUtf8，实际 {other:?}"),
    }
    let error = run_utf16(HostOp::Utf16ToCStr, &[0x61, 0x0000, 0x62]).expect_err("内嵌 NUL 必须失败");
    match error {
        OdinRuntimeError::EmbeddedNul { operation } => assert_eq!(operation, "utf16_to_c_str"),
        other => panic!("期望 EmbeddedNul，实际 {other:?}"),
    }
}

#[test]
fn c_str_symbols_reject_wrong_carriers_and_short_arguments() {
    for op in [HostOp::CStrToUtf8, HostOp::CStrToUtf16, HostOp::CStrLength] {
        let error = run(op, &[text(ASCII)]).expect_err("字符串不是 C 字符串载体");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "{op:?} 实际 {error:?}");
        let error = run(op, &[]).expect_err("缺参必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
    }
    for op in [HostOp::Utf8ToCStr, HostOp::Utf16ToCStr] {
        let error = run(op, &[Value::I32(1)]).expect_err("整数不是文本载体");
        assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "{op:?} 实际 {error:?}");
        let error = run(op, &[]).expect_err("缺参必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
    }

    // 非法字节槽：超出 u8 范围 / 非整数载体。
    let mut heap = ObjectHeap::new();
    for element in [Value::I32(0x100), Value::F64(1.0), Value::Null] {
        let array = array_runtime::build_fixed_array(&mut heap, 0, &[element.clone()]);
        for op in [HostOp::CStrToUtf8, HostOp::CStrToUtf16, HostOp::CStrLength] {
            let error = execute_host_op(op, &[array.clone()], &mut heap).expect_err("非法字节必须失败");
            assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "{op:?} 元素 {element:?} 实际 {error:?}");
        }
    }
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
        run_import("utf8.slice", vec![OdinConstant::String("a😀b".to_string()), OdinConstant::Integer32(1), OdinConstant::Integer32(1)], 3),
        Value::String("😀".to_string())
    );
}

#[test]
fn slice_out_of_bounds_fails_through_call_import() {
    let error =
        run_import_err("utf8.slice", vec![OdinConstant::String(ASCII.to_string()), OdinConstant::Integer32(0), OdinConstant::Integer32(9)], 3);
    match error {
        OdinRuntimeError::IndexOutOfBounds { operation, start, count, length } => {
            assert_eq!((operation, start, count, length), ("utf8.slice", 0, 9, 3));
        }
        other => panic!("期望 IndexOutOfBounds，实际 {other:?}"),
    }
}

#[test]
fn concat_executes_through_call_import() {
    assert_eq!(
        run_import("utf8.concat", vec![OdinConstant::String(EMPTY.to_string()), OdinConstant::String(ASTRAL.to_string())], 2),
        Value::String(ASTRAL.to_string())
    );
}

#[test]
fn eq_and_ne_execute_through_call_import() {
    assert_eq!(
        run_import("utf8.eq", vec![OdinConstant::String(COMBINING.to_string()), OdinConstant::String(COMBINING.to_string())], 2),
        Value::Bool(true)
    );
    assert_eq!(
        run_import("utf8.ne", vec![OdinConstant::String("e\u{0301}".to_string()), OdinConstant::String("\u{e9}".to_string())], 2),
        Value::Bool(true),
        "组合式与预组合式不等"
    );
}

#[test]
fn utf8_to_utf16_executes_through_call_import() {
    let value = run_import("utf8_to_utf16", vec![OdinConstant::String(ASTRAL.to_string())], 1);
    assert!(matches!(value, Value::Object(_)), "必须产出 UTF-16 数组载体，实际 {value:?}");
}

#[test]
fn utf16_to_utf8_executes_through_call_import() {
    assert_eq!(
        run_import_chain(vec![OdinConstant::String("a😀b".to_string())], vec!["utf8_to_utf16", "utf16_to_utf8"], &[(0, 1), (1, 1)]),
        text("a😀b"),
        "UTF-16 往返必须保持内容"
    );
}

#[test]
fn utf16_length_executes_through_call_import() {
    assert_eq!(
        run_import_chain(vec![OdinConstant::String(ASTRAL.to_string())], vec!["utf8_to_utf16", "utf16_length"], &[(0, 1), (1, 1)]),
        Value::I32(2),
        "astral 码点在 UTF-16 中是两个码元"
    );
    assert_eq!(
        run_import_chain(vec![OdinConstant::String(BMP_CHINESE.to_string())], vec!["utf8_to_utf16", "utf16_length"], &[(0, 1), (1, 1)]),
        Value::I32(2),
        "BMP 中文每字一个码元"
    );
}

#[test]
fn utf8_to_c_str_executes_through_call_import() {
    assert_eq!(
        run_import_chain(vec![OdinConstant::String(ASTRAL.to_string())], vec!["utf8_to_c_str", "c_str_length"], &[(0, 1), (1, 1)]),
        Value::I32(4),
        "astral 在 C 字符串里占 4 字节"
    );
}

#[test]
fn utf8_to_c_str_embedded_nul_fails_through_call_import() {
    let error = run_import_err("utf8_to_c_str", vec![OdinConstant::String("a\0b".to_string())], 1);
    match error {
        OdinRuntimeError::EmbeddedNul { operation } => assert_eq!(operation, "utf8_to_c_str"),
        other => panic!("期望 EmbeddedNul，实际 {other:?}"),
    }
}

#[test]
fn c_str_to_utf8_executes_through_call_import() {
    assert_eq!(
        run_import_chain(vec![OdinConstant::String("a😀b".to_string())], vec!["utf8_to_c_str", "c_str_to_utf8"], &[(0, 1), (1, 1)]),
        text("a😀b"),
        "C 字符串往返必须保持内容"
    );
}

#[test]
fn utf16_to_c_str_executes_through_call_import() {
    assert_eq!(
        run_import_chain(
            vec![OdinConstant::String(ASTRAL.to_string())],
            vec!["utf8_to_utf16", "utf16_to_c_str", "c_str_to_utf8"],
            &[(0, 1), (1, 1), (2, 1)]
        ),
        text(ASTRAL),
        "UTF-16 → C 字符串 → UTF-8 必须往返"
    );
}

#[test]
fn c_str_to_utf16_executes_through_call_import() {
    assert_eq!(
        run_import_chain(
            vec![OdinConstant::String(ASTRAL.to_string())],
            vec!["utf8_to_c_str", "c_str_to_utf16", "utf16_length"],
            &[(0, 1), (1, 1), (2, 1)]
        ),
        Value::I32(2),
        "C 字符串 → UTF-16 后 astral 仍是两个码元"
    );
}
