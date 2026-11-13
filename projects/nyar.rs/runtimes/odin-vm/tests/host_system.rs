//! 第 4 层：console / time / random / math 宿主符号族。

use acorn_odin::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinImport, OdinImportKind, OdinModuleData, emit_imm1,
    emit_imm2, emit_plain, encode_module,
};
use odin_vm::{
    ObjectHeap, Value,
    error::OdinRuntimeError,
    host::{HostOp, execute_host_op},
    module::LoadedModule,
    output_runtime::capture,
    random_runtime,
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

/// 随机测试串行锁：环境变量是进程级状态，涉及 `ODIN_VM_RAND_SEED` 的用例必须互斥。
static RANDOM_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// xoshiro256**（256 位状态，SplitMix64 播种）在固定种子下的逐值序列。
/// 换算法或改播种必须同步修改本表——这张表就是确定性证据。
const SEED_ZERO_SEQUENCE: [u64; 4] = [11091344671253066420, 13793997310169335082, 1900383378846508768, 7684712102626143532];
const SEED_ONE_SEQUENCE: [u64; 4] = [12966619160104079557, 9600361134598540522, 10590380919521690900, 7218738570589545383];
const SEED_DEFAULT_SEQUENCE: [u64; 4] = [17006091401577628144, 18369550347859807172, 17024098124861754899, 2308077655910561512];

#[test]
fn random_u64_matches_xoshiro256_star_star_fixed_seed_sequences() {
    for (seed, expected) in [(0u64, SEED_ZERO_SEQUENCE), (1u64, SEED_ONE_SEQUENCE), (random_runtime::DEFAULT_SEED, SEED_DEFAULT_SEQUENCE)] {
        assert_eq!(random_runtime::sequence_from_seed(seed, 4), expected, "seed {seed:#018X}");
    }
}

#[test]
fn random_u64_is_deterministic_across_vm_runs_with_explicit_seed() {
    let module = import_module("random_u64", Vec::new(), 0);
    let mut vm = OdinVm::new();
    vm.set_random_seed(0);
    let mut observed = Vec::new();
    for _ in 0..4 {
        let value = vm.run(&module, "main", Vec::new()).expect("随机调用必须成功");
        let Value::I64(bits) = value
        else {
            panic!("random_u64 必须返回 i64 位模式，实际 {value:?}");
        };
        observed.push(bits as u64);
    }
    assert_eq!(observed, SEED_ZERO_SEQUENCE, "显式种子下的序列必须与固定表一致");

    // 换一个 VM、同一显式种子：首值必须复现。
    let mut other = OdinVm::new();
    other.set_random_seed(1);
    let value = other.run(&module, "main", Vec::new()).expect("随机调用必须成功");
    let Value::I64(bits) = value
    else {
        panic!("random_u64 必须返回 i64 位模式，实际 {value:?}");
    };
    assert_eq!(bits as u64, SEED_ONE_SEQUENCE[0], "同种子必须复现首值");
}

#[test]
fn random_u64_default_seed_is_used_only_without_override() {
    let _guard = RANDOM_ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let previous = std::env::var_os(random_runtime::RANDOM_SEED_ENV);
    unsafe { std::env::remove_var(random_runtime::RANDOM_SEED_ENV) };
    // 在新线程取首值：保证线程局部随机状态未被子序列用例推进。
    let value = std::thread::spawn(|| run_ok(HostOp::RandomU64, &[])).join().expect("随机调用必须成功");
    let Value::I64(bits) = value
    else {
        panic!("random_u64 必须返回 i64 位模式，实际 {value:?}");
    };
    assert_eq!(bits as u64, SEED_DEFAULT_SEQUENCE[0], "无覆盖时必须用默认固定种子");
    if let Some(previous) = previous {
        unsafe { std::env::set_var(random_runtime::RANDOM_SEED_ENV, previous) };
    }
}

#[test]
fn random_u64_env_seed_overrides_default_and_invalid_values_fail() {
    let _guard = RANDOM_ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let previous = std::env::var_os(random_runtime::RANDOM_SEED_ENV);
    unsafe { std::env::set_var(random_runtime::RANDOM_SEED_ENV, "7") };
    // 在新线程取首值：线程局部状态首次初始化时才读取环境变量。
    let value = std::thread::spawn(|| run_ok(HostOp::RandomU64, &[])).join().expect("随机调用必须成功");
    let Value::I64(bits) = value
    else {
        panic!("random_u64 必须返回 i64 位模式，实际 {value:?}");
    };
    assert_eq!(bits as u64, random_runtime::sequence_from_seed(7, 1)[0], "环境变量种子必须覆盖默认种子");

    for invalid in ["abc", "-1", "0x10", "18446744073709551616", ""] {
        let error = random_runtime::parse_seed(random_runtime::RANDOM_SEED_ENV, invalid).expect_err("非法种子必须结构化失败");
        assert!(
            matches!(error, OdinRuntimeError::InvalidSeed { source, ref value } if source == random_runtime::RANDOM_SEED_ENV && value == invalid),
            "种子 {invalid:?} 实际 {error:?}"
        );
    }

    // 真实宿主路径：非法环境变量必须让 `random_u64` 结构化失败，而不是静默用默认种子。
    unsafe { std::env::set_var(random_runtime::RANDOM_SEED_ENV, "abc") };
    let error = std::thread::spawn(|| run(HostOp::RandomU64, &[])).join().expect("线程必须结束").expect_err("非法种子必须失败");
    assert!(
        matches!(error, OdinRuntimeError::InvalidSeed { source, ref value } if source == random_runtime::RANDOM_SEED_ENV && value == "abc"),
        "实际 {error:?}"
    );

    if let Some(previous) = previous {
        unsafe { std::env::set_var(random_runtime::RANDOM_SEED_ENV, previous) };
    }
    else {
        unsafe { std::env::remove_var(random_runtime::RANDOM_SEED_ENV) };
    }
}

#[test]
fn random_seed_priority_is_explicit_then_env_then_default() {
    assert_eq!(random_runtime::resolve_seed(Some(5), Some("bad")).expect("显式种子优先"), 5);
    assert_eq!(random_runtime::resolve_seed(None, Some("9")).expect("环境变量可解析"), 9);
    assert_eq!(random_runtime::resolve_seed(None, None).expect("默认种子"), random_runtime::DEFAULT_SEED);
    let error = random_runtime::resolve_seed(None, Some("bad")).expect_err("非法环境变量不得回落默认值");
    assert!(matches!(error, OdinRuntimeError::InvalidSeed { source, .. } if source == random_runtime::RANDOM_SEED_ENV));
}

fn f64_of(value: Value) -> f64 {
    match value {
        Value::F64(number) => number,
        other => panic!("期望 f64，实际 {other:?}"),
    }
}

#[test]
fn math_sqrt_matches_ieee_754_expected_values() {
    assert!(f64_of(run_ok(HostOp::MathSqrt, &[Value::F64(-1.0)])).is_nan(), "sqrt(-1.0) 必须是 NaN");
    let negative_zero = f64_of(run_ok(HostOp::MathSqrt, &[Value::F64(-0.0)]));
    assert_eq!(negative_zero, 0.0, "sqrt(-0.0) 数值为 0");
    assert!(negative_zero.is_sign_negative(), "sqrt(-0.0) 必须保留负号位，实际 {negative_zero:?}");

    let four = f64_of(run_ok(HostOp::MathSqrt, &[Value::F64(4.0)]));
    assert_eq!(four, 2.0, "sqrt(4.0) 必须是精确 2.0");

    let positive_zero = f64_of(run_ok(HostOp::MathSqrt, &[Value::F64(0.0)]));
    assert_eq!(positive_zero, 0.0, "sqrt(0.0) 数值为 0");
    assert!(!positive_zero.is_sign_negative(), "sqrt(0.0) 为正零");

    assert_eq!(f64_of(run_ok(HostOp::MathSqrt, &[Value::F64(f64::INFINITY)])), f64::INFINITY, "sqrt(+inf) 保持 +inf");
}

#[test]
fn math_sin_and_cos_match_ieee_754_expected_values() {
    let positive_zero = f64_of(run_ok(HostOp::MathSin, &[Value::F64(0.0)]));
    assert_eq!(positive_zero, 0.0, "sin(+0.0) 数值为 0");
    assert!(!positive_zero.is_sign_negative(), "sin(+0.0) 为正零");

    let negative_zero = f64_of(run_ok(HostOp::MathSin, &[Value::F64(-0.0)]));
    assert_eq!(negative_zero, 0.0, "sin(-0.0) 数值为 0");
    assert!(negative_zero.is_sign_negative(), "sin(-0.0) 必须保留负号位，实际 {negative_zero:?}");

    assert!(f64_of(run_ok(HostOp::MathSin, &[Value::F64(f64::INFINITY)])).is_nan(), "sin(+inf) 必须是 NaN");
    assert!(f64_of(run_ok(HostOp::MathSin, &[Value::F64(f64::NEG_INFINITY)])).is_nan(), "sin(-inf) 必须是 NaN");

    assert_eq!(f64_of(run_ok(HostOp::MathCos, &[Value::F64(0.0)])), 1.0, "cos(0.0) 必须是精确 1.0");
    assert!(f64_of(run_ok(HostOp::MathCos, &[Value::F64(f64::INFINITY)])).is_nan(), "cos(+inf) 必须是 NaN");
    assert!(f64_of(run_ok(HostOp::MathCos, &[Value::F64(f64::NEG_INFINITY)])).is_nan(), "cos(-inf) 必须是 NaN");
}

#[test]
fn math_symbols_execute_through_call_import_without_trapping() {
    let sqrt_two = f64_of(run_import("math_sqrt", vec![OdinConstant::Float64(4.0)], 1));
    assert_eq!(sqrt_two, 2.0);
    let sin_zero = f64_of(run_import("math_sin", vec![OdinConstant::Float64(0.0)], 1));
    assert_eq!(sin_zero, 0.0);
    assert!(!sin_zero.is_sign_negative());
    let cos_zero = f64_of(run_import("math_cos", vec![OdinConstant::Float64(0.0)], 1));
    assert_eq!(cos_zero, 1.0);
    // IEEE 域外输入也走 Ok，不 trap：宿主返回 NaN 而不是错误。
    let sqrt_negative = f64_of(run_import("math_sqrt", vec![OdinConstant::Float64(-1.0)], 1));
    assert!(sqrt_negative.is_nan());
    let sin_infinity = f64_of(run_import("math_sin", vec![OdinConstant::Float64(f64::INFINITY)], 1));
    assert!(sin_infinity.is_nan());
}

#[test]
fn math_symbols_reject_non_f64_carriers_and_short_arguments() {
    for op in [HostOp::MathSin, HostOp::MathCos, HostOp::MathSqrt] {
        for carrier in [Value::I32(0), Value::I64(0), Value::Null, Value::Bool(true)] {
            let error = run(op, &[carrier.clone()]).expect_err("非 f64 载体必须失败");
            assert!(matches!(error, OdinRuntimeError::TypeMismatch { .. }), "{op:?} 载体 {carrier:?} 实际 {error:?}");
        }
        let error = run(op, &[]).expect_err("缺参必须失败");
        assert!(matches!(error, OdinRuntimeError::StackUnderflow), "{op:?} 实际 {error:?}");
    }
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
