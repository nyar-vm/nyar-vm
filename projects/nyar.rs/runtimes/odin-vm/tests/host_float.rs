//! 第 2 层：`f64` 宿主符号族（九个）的真实执行。
//!
//! 语义合同（跨 lane 一致性硬约束）：
//! - `add/sub/mul/div/neg` 严格 IEEE-754，不 trap：除零得 ±inf / NaN，不返回 `Err`。
//! - 浮点转整数统一「饱和 + NaN→0」（wasm `trunc_sat` / JVM 窄化口径），有限值朝零截断。
//! - `i64_to_f64` 承认精度丢失（超过 2^53 的奇数按就近舍入）；`i32_to_f64` 全域精确。
//!
//! 每个符号至少有一条走真实 `CallImport` 的端到端路径。

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

fn run_f64(op: HostOp, args: &[Value]) -> f64 {
    match run(op, args).unwrap_or_else(|error| panic!("{op:?} 必须成功，实际 {error:?}")) {
        Value::F64(value) => value,
        other => panic!("{op:?} 必须返回 F64，实际 {other:?}"),
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
        name: "host_float".to_string(),
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
fn float_symbols_are_registered() {
    assert_eq!(parse_host_op("i32_to_f64"), Some(HostOp::I32ToF64));
    assert_eq!(parse_host_op("i64_to_f64"), Some(HostOp::I64ToF64));
    assert_eq!(parse_host_op("u32_to_f64"), Some(HostOp::U32ToF64));
    assert_eq!(parse_host_op("u64_to_f64"), Some(HostOp::U64ToF64));
    assert_eq!(parse_host_op("f64_to_i32_checked"), Some(HostOp::F64ToI32Checked));
    assert_eq!(parse_host_op("f64_to_u32_checked"), Some(HostOp::F64ToU32Checked));
    assert_eq!(parse_host_op("f64_to_i64_checked"), Some(HostOp::F64ToI64Checked));
    assert_eq!(parse_host_op("f64_to_u64_checked"), Some(HostOp::F64ToU64Checked));
    assert_eq!(parse_host_op("f64_add"), Some(HostOp::F64Add));
    assert_eq!(parse_host_op("f64_sub"), Some(HostOp::F64Sub));
    assert_eq!(parse_host_op("f64_mul"), Some(HostOp::F64Mul));
    assert_eq!(parse_host_op("f64_div"), Some(HostOp::F64Div));
    assert_eq!(parse_host_op("f64_neg"), Some(HostOp::F64Neg));
    assert_eq!(parse_host_op("f64_to_i32"), Some(HostOp::F64ToI32));
    assert_eq!(parse_host_op("f64_to_i64"), Some(HostOp::F64ToI64));
}

#[test]
fn f64_arithmetic_is_ieee754_without_trapping() {
    let one = Value::F64(1.0);
    assert_eq!(run_f64(HostOp::F64Add, &[Value::F64(1.5), Value::F64(2.25)]), 3.75);
    assert_eq!(run_f64(HostOp::F64Sub, &[Value::F64(5.5), Value::F64(2.25)]), 3.25);
    assert_eq!(run_f64(HostOp::F64Mul, &[Value::F64(3.0), Value::F64(0.5)]), 1.5);
    assert_eq!(run_f64(HostOp::F64Div, &[Value::F64(7.0), Value::F64(2.0)]), 3.5);
    assert_eq!(run_f64(HostOp::F64Neg, &[Value::F64(1.25)]), -1.25);

    // 除零不 trap：0/0 = NaN、1/0 = +inf、-1/0 = -inf、1/-0 = -inf。
    let zero_over_zero = run_f64(HostOp::F64Div, &[Value::F64(0.0), Value::F64(0.0)]);
    assert!(zero_over_zero.is_nan(), "0.0 / 0.0 必须是 NaN，实际 {zero_over_zero}");
    let positive_inf = run_f64(HostOp::F64Div, &[one.clone(), Value::F64(0.0)]);
    assert!(positive_inf.is_infinite() && positive_inf.is_sign_positive(), "1.0 / 0.0 必须是 +inf，实际 {positive_inf}");
    let negative_inf = run_f64(HostOp::F64Div, &[Value::F64(-1.0), Value::F64(0.0)]);
    assert!(negative_inf.is_infinite() && negative_inf.is_sign_negative(), "-1.0 / 0.0 必须是 -inf，实际 {negative_inf}");
    let negative_zero_divisor = run_f64(HostOp::F64Div, &[one.clone(), Value::F64(-0.0)]);
    assert!(negative_zero_divisor.is_infinite() && negative_zero_divisor.is_sign_negative(), "1.0 / -0.0 必须是 -inf");

    // -0.0 符号位：neg(0.0) = -0.0，neg(-0.0) = 0.0；0.0 + (-0.0) = +0.0。
    let negated_zero = run_f64(HostOp::F64Neg, &[Value::F64(0.0)]);
    assert!(negated_zero == 0.0 && negated_zero.is_sign_negative(), "neg(0.0) 必须是 -0.0，实际 {negated_zero}");
    let negated_negative_zero = run_f64(HostOp::F64Neg, &[Value::F64(-0.0)]);
    assert!(negated_negative_zero == 0.0 && negated_negative_zero.is_sign_positive(), "neg(-0.0) 必须是 +0.0");
    let sum_of_zeros = run_f64(HostOp::F64Add, &[Value::F64(0.0), Value::F64(-0.0)]);
    assert!(sum_of_zeros.is_sign_positive(), "0.0 + (-0.0) 必须是 +0.0（IEEE-754 round-to-nearest）");

    // NaN 传播：任一操作数为 NaN，结果必为 NaN（payload 不要求逐位一致）。
    let nan_add = run_f64(HostOp::F64Add, &[Value::F64(f64::NAN), Value::F64(1.0)]);
    assert!(nan_add.is_nan(), "NaN + 1.0 必须是 NaN");
    let nan_mul = run_f64(HostOp::F64Mul, &[Value::F64(2.0), Value::F64(f64::NAN)]);
    assert!(nan_mul.is_nan(), "2.0 * NaN 必须是 NaN");
    let nan_neg = run_f64(HostOp::F64Neg, &[Value::F64(f64::NAN)]);
    assert!(nan_neg.is_nan(), "neg(NaN) 必须是 NaN（只翻符号位）");
}

#[test]
fn f64_to_i32_saturates_and_maps_nan_to_zero() {
    let convert = |value: f64| match run(HostOp::F64ToI32, &[Value::F64(value)]).expect("有限/非有限 f64 都必须有定义结果") {
        Value::I32(value) => value,
        other => panic!("f64_to_i32 必须返回 I32，实际 {other:?}"),
    };

    assert_eq!(convert(f64::NAN), 0, "NaN → 0");
    assert_eq!(convert(f64::INFINITY), i32::MAX, "+inf → i32::MAX");
    assert_eq!(convert(f64::NEG_INFINITY), i32::MIN, "-inf → i32::MIN");
    assert_eq!(convert(1.0e30), i32::MAX, "超上界有限值饱和");
    assert_eq!(convert(-1.0e30), i32::MIN, "超下界有限值饱和");
    assert_eq!(convert(i32::MAX as f64), i32::MAX, "边界 i32::MAX 本身");
    assert_eq!(convert(i32::MIN as f64), i32::MIN, "边界 i32::MIN 本身");
    assert_eq!(convert(2_147_483_648.0), i32::MAX, "i32::MAX + 1 饱和");
    assert_eq!(convert(-2_147_483_649.0), i32::MIN, "i32::MIN - 1 饱和");
    assert_eq!(convert(0.0), 0);
    assert_eq!(convert(-0.0), 0);
    assert_eq!(convert(-1.5), -1, "朝零截断");
    assert_eq!(convert(1.9), 1, "朝零截断");
    assert_eq!(convert(2.5), 2, "朝零截断（不是就近舍入）");
}

#[test]
fn f64_to_i64_saturates_and_maps_nan_to_zero() {
    let convert = |value: f64| match run(HostOp::F64ToI64, &[Value::F64(value)]).expect("有限/非有限 f64 都必须有定义结果") {
        Value::I64(value) => value,
        other => panic!("f64_to_i64 必须返回 I64，实际 {other:?}"),
    };

    assert_eq!(convert(f64::NAN), 0, "NaN → 0");
    assert_eq!(convert(f64::INFINITY), i64::MAX, "+inf → i64::MAX");
    assert_eq!(convert(f64::NEG_INFINITY), i64::MIN, "-inf → i64::MIN");
    assert_eq!(convert(1.0e300), i64::MAX, "超上界有限值饱和");
    assert_eq!(convert(-1.0e300), i64::MIN, "超下界有限值饱和");
    // `i64::MAX as f64` 是 2^63，已超出 i64 表示范围：必须饱和到 MAX 而不是回绕。
    assert_eq!(convert(i64::MAX as f64), i64::MAX, "2^63 作为上界开区间，饱和到 i64::MAX");
    assert_eq!(convert(9_223_372_036_854_775_808.0), i64::MAX, "显式 2^63 同样饱和");
    assert_eq!(convert(-9_223_372_036_854_775_808.0), i64::MIN, "-2^63 恰在闭区间下界");
    assert_eq!(convert(9_223_372_036_854_774_784.0), 9_223_372_036_854_774_784, "2^63 - 1024 是 f64 可表示的最大 i64 值，不饱和");
    assert_eq!(convert(0.0), 0);
    assert_eq!(convert(-0.0), 0);
    assert_eq!(convert(-1.5), -1, "朝零截断");
    assert_eq!(convert(1.9), 1, "朝零截断");
}

#[test]
fn i64_to_f64_admits_rounding_beyond_two_pow_53() {
    let convert = |value: i64| match run(HostOp::I64ToF64, &[Value::I64(value)]).expect("i64 全域必须可转") {
        Value::F64(value) => value,
        other => panic!("i64_to_f64 必须返回 F64，实际 {other:?}"),
    };

    assert_eq!(convert(0), 0.0);
    assert_eq!(convert(-1), -1.0);
    let two_pow_53 = 1_i64 << 53;
    assert_eq!(convert(two_pow_53), 9_007_199_254_740_992.0, "2^53 精确");
    assert_eq!(convert(two_pow_53 - 1), 9_007_199_254_740_991.0, "2^53 - 1 精确");
    // 2^53 + 1 是奇数，超过 f64 尾数宽度；就近舍入（ties-to-even）落到 2^53。
    assert_eq!(convert(two_pow_53 + 1), 9_007_199_254_740_992.0, "2^53 + 1 舍入到 2^53");
    // i64::MAX 舍入到 2^63（已超出 i64 表示范围），这是合同承认的精度丢失。
    assert_eq!(convert(i64::MAX), 9_223_372_036_854_775_808.0, "i64::MAX → 2^63");
    assert_eq!(convert(i64::MIN), -9_223_372_036_854_775_808.0, "i64::MIN → -2^63 精确");
}

#[test]
fn i32_to_f64_is_exact_at_boundaries() {
    let convert = |value: i32| match run(HostOp::I32ToF64, &[Value::I32(value)]).expect("i32 全域必须可转") {
        Value::F64(value) => value,
        other => panic!("i32_to_f64 必须返回 F64，实际 {other:?}"),
    };

    assert_eq!(convert(0), 0.0);
    assert_eq!(convert(-1), -1.0);
    assert_eq!(convert(i32::MAX), 2_147_483_647.0, "i32::MAX 精确");
    assert_eq!(convert(i32::MIN), -2_147_483_648.0, "i32::MIN 精确");
    // 2^24 + 1 超出 i32→f32 的尾数宽度，但远在 f64 的 53 位内，必须精确。
    assert_eq!(convert(16_777_217), 16_777_217.0, "i32 全域在 f64 中精确");
}

#[test]
fn float_ops_reject_wrong_carriers_and_short_arguments() {
    // 非 f64 载体不得隐式转换（含 f32：不做静默加宽）。
    for (op, args) in [
        (HostOp::F64Add, vec![Value::I32(1), Value::F64(2.0)]),
        (HostOp::F64Sub, vec![Value::F64(1.0), Value::Bool(true)]),
        (HostOp::F64Mul, vec![Value::F64(1.0), Value::Null]),
        (HostOp::F64Div, vec![Value::I32(1), Value::I32(2)]),
        (HostOp::F64Neg, vec![Value::String("1.5".to_string())]),
        (HostOp::F64Neg, vec![Value::F32(1.5)]),
        (HostOp::F64ToI32, vec![Value::I64(1)]),
        (HostOp::F64ToI64, vec![Value::String("1.5".to_string())]),
    ] {
        let error = run(op, &args).expect_err("非 f64 载体必须结构化失败");
        match error {
            OdinRuntimeError::TypeMismatch { expected, .. } => assert_eq!(expected, "f64", "{op:?} 期望形态"),
            other => panic!("{op:?} 期望 TypeMismatch，实际 {other:?}"),
        }
    }
    let error = run(HostOp::I32ToF64, &[Value::Bool(true)]).expect_err("bool 不得转 f64");
    assert!(matches!(error, OdinRuntimeError::TypeMismatch { expected: "i32", .. }), "实际 {error:?}");
    let error = run(HostOp::I64ToF64, &[Value::F64(1.0)]).expect_err("f64 不得转 i64_to_f64");
    assert!(matches!(error, OdinRuntimeError::TypeMismatch { expected: "i64", .. }), "实际 {error:?}");

    // 参数个数不足：不得返回默认值。
    for op in [HostOp::F64Add, HostOp::F64Sub, HostOp::F64Mul, HostOp::F64Div] {
        let error = run(op, &[Value::F64(1.0)]).expect_err("缺第二操作数必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
        let error = run(op, &[]).expect_err("缺全部操作数必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
    }
    for op in [HostOp::F64Neg, HostOp::F64ToI32, HostOp::F64ToI64, HostOp::I32ToF64, HostOp::I64ToF64] {
        let error = run(op, &[]).expect_err("缺操作数必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
    }
}

// ---- 真实 `CallImport` 端到端：九个符号各至少一条 ----

#[test]
fn i32_to_f64_executes_through_call_import() {
    assert_eq!(run_import("i32_to_f64", vec![OdinConstant::Integer32(i32::MAX)], 1), Value::F64(2_147_483_647.0));
}

#[test]
fn i64_to_f64_executes_through_call_import() {
    // `i64::MIN` 用 i64_shl 构造（常量池没有 Integer64）：1 << 63。
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Constant, 0);
    emit_imm1(&mut code, OdinHeadCode::Constant, 1);
    emit_imm2(&mut code, OdinHeadCode::CallImport, 0, 2); // i64_shl
    emit_imm2(&mut code, OdinHeadCode::CallImport, 1, 1); // i64_to_f64
    emit_plain(&mut code, OdinHeadCode::Return);
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "i64_to_f64_e2e".to_string(),
        constants: vec![OdinConstant::Integer32(1), OdinConstant::Integer32(63)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: vec![
            OdinImport { kind: OdinImportKind::Function, module_name: "nyar.host".to_string(), symbol_name: "i64_shl".to_string() },
            OdinImport { kind: OdinImportKind::Function, module_name: "nyar.host".to_string(), symbol_name: "i64_to_f64".to_string() },
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
    assert_eq!(value, Value::F64(-9_223_372_036_854_775_808.0));
}

#[test]
fn f64_add_executes_through_call_import() {
    assert_eq!(run_import("f64_add", vec![OdinConstant::Float64(1.5), OdinConstant::Float64(2.25)], 2), Value::F64(3.75));
}

#[test]
fn f64_sub_executes_through_call_import() {
    assert_eq!(run_import("f64_sub", vec![OdinConstant::Float64(5.5), OdinConstant::Float64(2.25)], 2), Value::F64(3.25));
}

#[test]
fn f64_mul_executes_through_call_import() {
    assert_eq!(run_import("f64_mul", vec![OdinConstant::Float64(3.0), OdinConstant::Float64(0.5)], 2), Value::F64(1.5));
}

#[test]
fn f64_div_by_zero_executes_through_call_import() {
    let value = run_import("f64_div", vec![OdinConstant::Float64(1.0), OdinConstant::Float64(0.0)], 2);
    match value {
        Value::F64(value) => assert!(value.is_infinite() && value.is_sign_positive(), "1.0 / 0.0 必须是 +inf，实际 {value}"),
        other => panic!("f64_div 必须返回 F64，实际 {other:?}"),
    }
}

#[test]
fn f64_neg_executes_through_call_import() {
    assert_eq!(run_import("f64_neg", vec![OdinConstant::Float64(1.25)], 1), Value::F64(-1.25));
}

#[test]
fn f64_to_i32_saturates_through_call_import() {
    assert_eq!(run_import("f64_to_i32", vec![OdinConstant::Float64(1.0e30)], 1), Value::I32(i32::MAX));
    assert_eq!(run_import("f64_to_i32", vec![OdinConstant::Float64(f64::NAN)], 1), Value::I32(0), "NaN 常量走 CallImport 也必须落到 0");
}

#[test]
fn f64_to_i64_saturates_through_call_import() {
    assert_eq!(run_import("f64_to_i64", vec![OdinConstant::Float64(-1.0e300)], 1), Value::I64(i64::MIN));
    assert_eq!(run_import("f64_to_i64", vec![OdinConstant::Float64(f64::INFINITY)], 1), Value::I64(i64::MAX));
}

// ---- 无符号整数转 f64：按无符号位模式解释 ----

#[test]
fn u32_to_f64_interprets_unsigned_bit_pattern() {
    let convert = |value: i32| match run(HostOp::U32ToF64, &[Value::I32(value)]).expect("i32 全域必须可转") {
        Value::F64(value) => value,
        other => panic!("u32_to_f64 必须返回 F64，实际 {other:?}"),
    };

    assert_eq!(convert(0), 0.0);
    assert_eq!(convert(-1), 4_294_967_295.0, "i32 位模式 -1 按 u32 解释是 u32::MAX");
    assert_eq!(convert(i32::MIN), 2_147_483_648.0, "i32::MIN 按 u32 解释是 2^31");
    assert_eq!(convert(i32::MAX), 2_147_483_647.0);
    let error = run(HostOp::U32ToF64, &[Value::I64(1)]).expect_err("非 i32 载体必须结构化失败");
    assert!(matches!(error, OdinRuntimeError::TypeMismatch { expected: "i32", .. }), "实际 {error:?}");
}

#[test]
fn u64_to_f64_interprets_unsigned_bit_pattern() {
    let convert = |value: i64| match run(HostOp::U64ToF64, &[Value::I64(value)]).expect("i64 载体全域必须可转") {
        Value::F64(value) => value,
        other => panic!("u64_to_f64 必须返回 F64，实际 {other:?}"),
    };

    assert_eq!(convert(0), 0.0);
    assert_eq!(convert(1), 1.0);
    assert_eq!(convert(i64::MIN), 9_223_372_036_854_775_808.0, "2^63 必须为正（无符号解释）");
    assert_eq!(convert(-1), 18_446_744_073_709_551_616.0, "u64::MAX 就近舍入到 2^64");
}

#[test]
fn u64_to_f64_executes_through_call_import() {
    // `u64::MAX` 无法直接静态表示：用 `1 << 63` 构造 2^63（有符号解释会是 i64::MIN）。
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Constant, 0);
    emit_imm1(&mut code, OdinHeadCode::Constant, 1);
    emit_imm2(&mut code, OdinHeadCode::CallImport, 0, 2); // i64_shl
    emit_imm2(&mut code, OdinHeadCode::CallImport, 1, 1); // u64_to_f64
    emit_plain(&mut code, OdinHeadCode::Return);
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "u64_to_f64_e2e".to_string(),
        constants: vec![OdinConstant::Integer32(1), OdinConstant::Integer32(63)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: vec![
            OdinImport { kind: OdinImportKind::Function, module_name: "nyar.host".to_string(), symbol_name: "i64_shl".to_string() },
            OdinImport { kind: OdinImportKind::Function, module_name: "nyar.host".to_string(), symbol_name: "u64_to_f64".to_string() },
        ],
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    let module = LoadedModule::from_bytes(&encode_module(&data)).expect("模块必须加载成功");
    assert_eq!(
        OdinVm::new().run(&module, "main", Vec::new()).expect("端到端执行"),
        Value::F64(9_223_372_036_854_775_808.0),
        "2^63 走真实 CallImport 必须保持正号"
    );
}

// ---- f64 → 整数 checked：朝零截断，NaN / 越界结构化失败 ----

#[test]
fn f64_to_i32_checked_truncates_and_fails_out_of_range() {
    let convert = |value: f64| match run(HostOp::F64ToI32Checked, &[Value::F64(value)]).expect("界内必须成功") {
        Value::I32(value) => value,
        other => panic!("f64_to_i32_checked 必须返回 I32，实际 {other:?}"),
    };

    assert_eq!(convert(2.75), 2, "朝零截断");
    assert_eq!(convert(-2.75), -2, "朝零截断（不是向下取整）");
    assert_eq!(convert(2_147_483_647.9), i32::MAX, "截断后仍在界内的上边界");
    assert_eq!(convert(-2_147_483_648.9), i32::MIN, "截断后落在闭区间下界");

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 2_147_483_648.0, -2_147_483_649.0, 1.0e300] {
        let error = run(HostOp::F64ToI32Checked, &[Value::F64(bad)]).expect_err("NaN / 越界必须结构化失败");
        assert!(matches!(error, OdinRuntimeError::ArithmeticOverflow { .. }), "{bad} 实际 {error:?}");
    }
}

#[test]
fn f64_to_u32_checked_rejects_negative_and_overflow() {
    let convert = |value: f64| match run(HostOp::F64ToU32Checked, &[Value::F64(value)]).expect("界内必须成功") {
        Value::I32(value) => value,
        other => panic!("f64_to_u32_checked 必须返回 I32，实际 {other:?}"),
    };

    assert_eq!(convert(0.0), 0);
    assert_eq!(convert(123.9), 123);
    assert_eq!(convert(-0.5), 0, "朝零截断得到 -0，仍在无符号界内");
    assert_eq!(convert(4_294_967_295.0), -1, "u32::MAX 以 i32 位模式 -1 承载");

    for bad in [f64::NAN, -1.0, 4_294_967_296.0, f64::INFINITY, f64::NEG_INFINITY] {
        let error = run(HostOp::F64ToU32Checked, &[Value::F64(bad)]).expect_err("负值 / 越界必须结构化失败");
        assert!(matches!(error, OdinRuntimeError::ArithmeticOverflow { .. }), "{bad} 实际 {error:?}");
    }
}

#[test]
fn f64_to_i64_checked_truncates_and_fails_out_of_range() {
    let convert = |value: f64| match run(HostOp::F64ToI64Checked, &[Value::F64(value)]).expect("界内必须成功") {
        Value::I64(value) => value,
        other => panic!("f64_to_i64_checked 必须返回 I64，实际 {other:?}"),
    };

    assert_eq!(convert(2.75), 2, "朝零截断");
    assert_eq!(convert(-2.75), -2, "朝零截断（不是向下取整）");
    assert_eq!(convert(-9_223_372_036_854_775_808.0), i64::MIN, "-2^63 恰在闭区间下界");
    assert_eq!(convert(9_223_372_036_854_774_784.0), 9_223_372_036_854_774_784, "2^63 - 1024 是 f64 可表示的最大 i64 值");

    for bad in [f64::NAN, 9_223_372_036_854_775_808.0, -1.0e300, 1.0e300, f64::INFINITY] {
        let error = run(HostOp::F64ToI64Checked, &[Value::F64(bad)]).expect_err("NaN / 越界必须结构化失败");
        assert!(matches!(error, OdinRuntimeError::ArithmeticOverflow { .. }), "{bad} 实际 {error:?}");
    }
}

#[test]
fn f64_to_u64_checked_rejects_negative_and_overflow() {
    let convert = |value: f64| match run(HostOp::F64ToU64Checked, &[Value::F64(value)]).expect("界内必须成功") {
        Value::I64(value) => value,
        other => panic!("f64_to_u64_checked 必须返回 I64，实际 {other:?}"),
    };

    assert_eq!(convert(0.0), 0);
    assert_eq!(convert(123.9), 123);
    assert_eq!(convert(-0.5), 0, "朝零截断得到 -0，仍在无符号界内");
    assert_eq!(convert(18_446_744_073_709_549_568.0), (18_446_744_073_709_549_568_u64) as i64, "2^64 - 2048 可表示");

    for bad in [f64::NAN, -1.0, 18_446_744_073_709_551_616.0, 1.0e300, f64::NEG_INFINITY] {
        let error = run(HostOp::F64ToU64Checked, &[Value::F64(bad)]).expect_err("负值 / 越界必须结构化失败");
        assert!(matches!(error, OdinRuntimeError::ArithmeticOverflow { .. }), "{bad} 实际 {error:?}");
    }
}

#[test]
fn f64_to_i64_checked_executes_through_call_import() {
    assert_eq!(run_import("f64_to_i64_checked", vec![OdinConstant::Float64(-2.75)], 1), Value::I64(-2));
}

#[test]
fn f64_to_i64_checked_traps_out_of_range_through_call_import() {
    let error = run_import_err("f64_to_i64_checked", vec![OdinConstant::Float64(1.0e30)], 1);
    assert!(
        matches!(error, OdinRuntimeError::ArithmeticOverflow { operation: "host f64_to_i64_checked" }),
        "越界必须走结构化失败，实际 {error:?}"
    );
}
