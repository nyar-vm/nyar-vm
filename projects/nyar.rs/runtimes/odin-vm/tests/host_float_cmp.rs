//! 第 2b 层 A：`f64` 比较族宿主符号（`f64_eq`/`ne`/`lt`/`le`/`gt`/`ge`）。
//!
//! 语义按 IEEE-754 写死：
//! - `NaN` 与任何值（含 NaN 自身）：`eq`/`lt`/`le`/`gt`/`ge` 全 `false`，`ne` 为 `true`；
//! - `+0.0` 与 `-0.0`：`eq` 为 `true`（数值相等，不是按位比较）；
//! - `±inf` 参与比较按序数正常；
//! - 返回载体统一为 `Value::Bool`（与 `i64_eq` 等既有比较符号一致）。
//!
//! adaptor 侧尚未声明这些符号，V 侧暂不可达；本层只把 VM 侧能力做实。

use acorn_odin::{
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

fn compare(op: HostOp, lhs: f64, rhs: f64) -> bool {
    match run(op, &[Value::F64(lhs), Value::F64(rhs)]).unwrap_or_else(|error| panic!("{op:?} 必须成功，实际 {error:?}")) {
        Value::Bool(value) => value,
        other => panic!("{op:?} 必须返回 Bool 载体，实际 {other:?}"),
    }
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
        name: "host_float_cmp".to_string(),
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

fn run_import(symbol: &str, lhs: f64, rhs: f64) -> Value {
    let module = import_module(symbol, vec![OdinConstant::Float64(lhs), OdinConstant::Float64(rhs)], 2);
    OdinVm::new().run(&module, "main", Vec::new()).expect("端到端执行必须成功")
}

#[test]
fn f64_comparison_symbols_are_registered() {
    assert_eq!(parse_host_op("f64_eq"), Some(HostOp::F64Eq));
    assert_eq!(parse_host_op("f64_ne"), Some(HostOp::F64Ne));
    assert_eq!(parse_host_op("f64_lt"), Some(HostOp::F64Lt));
    assert_eq!(parse_host_op("f64_le"), Some(HostOp::F64Le));
    assert_eq!(parse_host_op("f64_gt"), Some(HostOp::F64Gt));
    assert_eq!(parse_host_op("f64_ge"), Some(HostOp::F64Ge));
}

#[test]
fn f64_ordering_comparisons() {
    // 1.0 vs 2.0：小于侧为真，大于侧为假。
    assert!(compare(HostOp::F64Lt, 1.0, 2.0));
    assert!(!compare(HostOp::F64Gt, 1.0, 2.0));
    assert!(compare(HostOp::F64Le, 1.0, 2.0));
    assert!(!compare(HostOp::F64Ge, 1.0, 2.0));
    assert!(!compare(HostOp::F64Eq, 1.0, 2.0));
    assert!(compare(HostOp::F64Ne, 1.0, 2.0));

    // 相等：eq/ne/le/ge 两侧都成立，严格序关系不成立。
    assert!(compare(HostOp::F64Eq, 2.0, 2.0));
    assert!(!compare(HostOp::F64Ne, 2.0, 2.0));
    assert!(compare(HostOp::F64Le, 2.0, 2.0));
    assert!(compare(HostOp::F64Ge, 2.0, 2.0));
    assert!(!compare(HostOp::F64Lt, 2.0, 2.0));
    assert!(!compare(HostOp::F64Gt, 2.0, 2.0));
}

#[test]
fn f64_zero_sign_does_not_affect_equality() {
    // IEEE-754 相等按数值比较：+0.0 与 -0.0 相等，且不小于/大于对方。
    assert!(compare(HostOp::F64Eq, 0.0, -0.0));
    assert!(!compare(HostOp::F64Ne, 0.0, -0.0));
    assert!(compare(HostOp::F64Le, 0.0, -0.0));
    assert!(compare(HostOp::F64Ge, 0.0, -0.0));
    assert!(!compare(HostOp::F64Lt, 0.0, -0.0));
    assert!(!compare(HostOp::F64Gt, 0.0, -0.0));
}

#[test]
fn f64_nan_is_unordered_and_never_equal() {
    let nan = f64::NAN;
    // NaN 与有限值。
    for op in [HostOp::F64Eq, HostOp::F64Lt, HostOp::F64Le, HostOp::F64Gt, HostOp::F64Ge] {
        assert!(!compare(op, nan, 1.0), "{op:?}(NaN, 1.0) 必须为 false");
        assert!(!compare(op, 1.0, nan), "{op:?}(1.0, NaN) 必须为 false");
    }
    assert!(compare(HostOp::F64Ne, nan, 1.0));
    assert!(compare(HostOp::F64Ne, 1.0, nan));
    // NaN 与 NaN 自身：只有 ne 为 true。
    assert!(!compare(HostOp::F64Eq, nan, nan));
    assert!(compare(HostOp::F64Ne, nan, nan));
    for op in [HostOp::F64Lt, HostOp::F64Le, HostOp::F64Gt, HostOp::F64Ge] {
        assert!(!compare(op, nan, nan), "{op:?}(NaN, NaN) 必须为 false");
    }
}

#[test]
fn f64_infinity_compares_by_order() {
    assert!(compare(HostOp::F64Gt, f64::INFINITY, 1.0e300));
    assert!(compare(HostOp::F64Lt, f64::NEG_INFINITY, -1.0e300));
    assert!(compare(HostOp::F64Gt, f64::INFINITY, f64::NEG_INFINITY));
    assert!(compare(HostOp::F64Eq, f64::INFINITY, f64::INFINITY));
    assert!(compare(HostOp::F64Ge, f64::INFINITY, f64::INFINITY));
    assert!(compare(HostOp::F64Le, f64::NEG_INFINITY, f64::NEG_INFINITY));
    assert!(!compare(HostOp::F64Lt, f64::INFINITY, f64::INFINITY));
}

#[test]
fn f64_comparison_returns_bool_carrier() {
    // 载体显式断言：VM 现有比较约定是 `Value::Bool`，不自创整数载体。
    assert_eq!(run(HostOp::F64Eq, &[Value::F64(1.0), Value::F64(1.0)]).expect("eq"), Value::Bool(true));
    assert_eq!(run(HostOp::F64Ne, &[Value::F64(1.0), Value::F64(2.0)]).expect("ne"), Value::Bool(true));
    assert_eq!(run(HostOp::F64Lt, &[Value::F64(1.0), Value::F64(2.0)]).expect("lt"), Value::Bool(true));
}

#[test]
fn f64_comparison_rejects_wrong_carriers_and_short_arguments() {
    let ops = [HostOp::F64Eq, HostOp::F64Ne, HostOp::F64Lt, HostOp::F64Le, HostOp::F64Gt, HostOp::F64Ge];
    for op in ops {
        // 非 f64 载体：I32 / Null / F32 / String 在任一操作数位置都必须结构化失败。
        for bad in [Value::I32(1), Value::Null, Value::F32(1.5), Value::String("1.0".to_string())] {
            let error = run(op, &[bad.clone(), Value::F64(2.0)]).expect_err("左操作数非 f64 必须失败");
            match error {
                OdinRuntimeError::TypeMismatch { expected, .. } => assert_eq!(expected, "f64", "{op:?} 左操作数"),
                other => panic!("{op:?} 期望 TypeMismatch，实际 {other:?}"),
            }
            let error = run(op, &[Value::F64(1.0), bad]).expect_err("右操作数非 f64 必须失败");
            match error {
                OdinRuntimeError::TypeMismatch { expected, .. } => assert_eq!(expected, "f64", "{op:?} 右操作数"),
                other => panic!("{op:?} 期望 TypeMismatch，实际 {other:?}"),
            }
        }
        // 缺参：不得返回默认 false。
        let error = run(op, &[Value::F64(1.0)]).expect_err("缺右操作数必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
        let error = run(op, &[]).expect_err("缺全部操作数必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
    }
}

// ---- 真实 `CallImport` 端到端：六个符号各一条 ----

#[test]
fn f64_eq_executes_through_call_import() {
    assert_eq!(run_import("f64_eq", 2.0, 2.0), Value::Bool(true));
}

#[test]
fn f64_ne_executes_through_call_import() {
    assert_eq!(run_import("f64_ne", 2.0, 3.0), Value::Bool(true));
}

#[test]
fn f64_lt_executes_through_call_import() {
    assert_eq!(run_import("f64_lt", 1.0, 2.0), Value::Bool(true));
}

#[test]
fn f64_le_executes_through_call_import() {
    assert_eq!(run_import("f64_le", 2.0, 2.0), Value::Bool(true));
}

#[test]
fn f64_gt_executes_through_call_import() {
    assert_eq!(run_import("f64_gt", 3.0, 2.0), Value::Bool(true));
}

#[test]
fn f64_ge_executes_through_call_import() {
    assert_eq!(run_import("f64_ge", 2.0, 2.0), Value::Bool(true));
}

#[test]
fn f64_eq_with_nan_executes_through_call_import() {
    // NaN 常量走真实导入：eq 必须为 false，ne 必须为 true。
    assert_eq!(run_import("f64_eq", f64::NAN, f64::NAN), Value::Bool(false));
    assert_eq!(run_import("f64_ne", f64::NAN, f64::NAN), Value::Bool(true));
}
