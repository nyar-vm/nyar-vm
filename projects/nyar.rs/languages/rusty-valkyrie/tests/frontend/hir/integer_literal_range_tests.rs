//! 整数字面量的符号/量级建模与上下文结算边界测试。
//!
//! 合同（架构归属 HIR）：
//! - 字面量在 HIR 中以 `i128` 精确承载符号与量级，构造时只拒绝超出 `u64` 的
//!   量级；目标宽度与有符号性由上下文在 `integer_literal::settle_integer` 一次结算。
//! - `i64` 上下文装不下时给出结构化诊断；`u64` 上下文可承载完整正半域。
//! - 无上下文时默认宽度为「能容纳该值的最小有符号宽度，否则 `u64`」。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    types::hir::{HirExprKind, HirLiteral, HirStatementKind},
};

/// 编译 `micro probe() { let ... }` 并返回首个 let 初始化表达式的 kind。
fn first_let_initializer_kind(source: &str) -> HirExprKind {
    let hir = ValkyrieCompiler::default().compile_source(source).expect("字面量必须完成 HIR lowering");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "probe").expect("缺少 probe 函数");
    let statement = function.body.statements.first().expect("缺少 let 语句");
    let HirStatementKind::Let { initializer: Some(initializer), .. } = &statement.kind
    else {
        panic!("首语句必须是带初始化的 let：{:?}", statement.kind)
    };
    initializer.kind.clone()
}

fn assert_integer(source: &str, expected: i128) {
    match first_let_initializer_kind(source) {
        HirExprKind::Literal(HirLiteral::Integer(value)) => assert_eq!(value, expected, "源码 {source}"),
        other => panic!("源码 {source} 必须归未定型整数字面量，实际 {other:?}"),
    }
}

/// 编译必须返回结构化范围诊断，而不是 panic 或 `unwrap` 崩溃。
fn expect_out_of_range(source: &str) -> (String, String, String, usize, usize) {
    let error = ValkyrieCompiler::default().compile_source(source).expect_err(&format!("源码 {source} 必须是结构化诊断"));
    match error {
        HirError::IntegerLiteralOutOfRange { literal, magnitude, range, span, .. } => (literal, magnitude, range, span.start, span.end),
        other => panic!("源码 {source} 必须是 IntegerLiteralOutOfRange，实际 {other:?}"),
    }
}

// ---------- 正例：符号与量级在 HIR 中精确保留 ----------

/// `i64::MIN` 的源码写法保持精确。
#[test]
fn lowest_i64_literal_is_negative_power_of_two() {
    assert_integer("micro probe() { let value = -9223372036854775808 }", i64::MIN as i128);
}

/// `i64::MAX` 仍按位精确。
#[test]
fn highest_i64_literal_is_exact() {
    assert_integer("micro probe() { let value = 9223372036854775807 }", i64::MAX as i128);
}

/// `i32` 边界经同一路径仍正常。
#[test]
fn i32_boundary_literal_stays_exact() {
    assert_integer("micro probe() { let value = -2147483648 }", -2_147_483_648);
}

/// `2^63` 超出 `i64::MAX`，但仍在 `u64` 正半域：HIR 不提前拒绝，
/// 是否可行由上下文结算（`u64` 合法，`i64` 报范围诊断）。
#[test]
fn positive_two_pow_63_is_preserved_until_context_settles() {
    assert_integer("micro probe() { let value = 9223372036854775808 }", 9_223_372_036_854_775_808);
}

/// `-2^63 - 1` 比 `i64::MIN` 更小，但 `i128` 可精确承载；上下文为 `i64` 时再拒绝。
#[test]
fn below_lowest_i64_is_preserved_until_context_settles() {
    assert_integer("micro probe() { let value = -9223372036854775809 }", -9_223_372_036_854_775_809);
}

/// `u64::MAX` 是完整正半域的上界，HIR 必须精确保留。
#[test]
fn highest_u64_literal_is_preserved() {
    assert_integer("micro probe() { let value = 18446744073709551615 }", u64::MAX as i128);
}

/// 大十六进制常量不再需要拆高低位。
#[test]
fn high_bit_hex_literal_is_preserved() {
    assert_integer("micro probe() { let value = 0xD807AA98A3030242 }", 0xD807AA98A3030242_u64 as i128);
}

// ---------- 反例：量级超出 u64，在构造处结构化失败 ----------

/// `-2^64` 连量级都超出 `u64`：失败发生在量级承载侧，仍是结构化诊断。
#[test]
fn negative_beyond_u64_magnitude_is_out_of_range() {
    let (literal, magnitude, range, start, end) = expect_out_of_range("micro probe() { let value = -18446744073709551616 }");
    assert_eq!(literal, "-18446744073709551616");
    assert_eq!(magnitude, "18446744073709551616");
    assert_eq!(range, "unsigned_64_magnitude");
    assert!(end > start, "诊断必须带真实 span，实际 {start}..{end}");
}
