//! 整数字面量按"符号 + 量级"建模的边界测试。
//!
//! 旧实现在取负之前按 `i64` 解析量级，`9223372036854775808` 先溢出并 panic。
//! 合同：量级用 `u64` 承载，符号在结合时应用，最后做 `i64` 范围检查；
//! `-2^63` 合法，`+2^63` 与 `-2^64` 都是结构化诊断。

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

fn assert_i64(source: &str, expected: i64) {
    match first_let_initializer_kind(source) {
        HirExprKind::Literal(HirLiteral::Integer64(value)) => assert_eq!(value, expected, "源码 {source}"),
        other => panic!("源码 {source} 必须归 i64 字面量，实际 {other:?}"),
    }
}

/// 编译必须返回结构化范围诊断，而不是 panic 或 `unwrap` 崩溃。
fn expect_out_of_range(source: &str) -> (String, String, String, usize, usize) {
    let error = ValkyrieCompiler::default().compile_source(source).expect_err(&format!("源码 {source} 必须是结构化诊断"));
    match error {
        HirError::IntegerLiteralOutOfRange { literal, magnitude, range, span, .. } => {
            (literal, magnitude, range, span.start, span.end)
        }
        other => panic!("源码 {source} 必须是 IntegerLiteralOutOfRange，实际 {other:?}"),
    }
}

// ---------- 正例：符号与量级结合后落在 `i64` 内 ----------

/// `i64::MIN` 的源码写法必须合法：量级是 `2^63`，取负后正好是 `i64::MIN`。
#[test]
fn lowest_i64_literal_is_negative_power_of_two() {
    assert_i64("micro probe() { let value = -9223372036854775808 }", i64::MIN);
}

/// `i64::MAX` 仍按位精确。
#[test]
fn highest_i64_literal_is_exact() {
    assert_i64("micro probe() { let value = 9223372036854775807 }", i64::MAX);
}

/// `i32` 边界经同一路径仍正常。
#[test]
fn i32_boundary_literal_stays_exact() {
    assert_i64("micro probe() { let value = -2147483648 }", -2_147_483_648);
}

// ---------- 反例：量级先于符号承载，失败是结构化诊断 ----------

/// 不带负号的 `2^63` 超出 `i64::MAX`：量级可被 `u64` 承载，范围检查失败。
#[test]
fn positive_two_pow_63_is_out_of_range() {
    let (literal, magnitude, range, start, end) = expect_out_of_range("micro probe() { let value = 9223372036854775808 }");
    assert_eq!(literal, "9223372036854775808");
    assert_eq!(magnitude, "9223372036854775808");
    assert_eq!(range, "signed_64");
    assert!(end > start, "诊断必须带真实 span，实际 {start}..{end}");
}

/// `-2^63 - 1` 超出 `i64::MIN`：量级仍可承载，范围检查失败。
#[test]
fn below_lowest_i64_is_out_of_range() {
    let (literal, magnitude, range, start, end) = expect_out_of_range("micro probe() { let value = -9223372036854775809 }");
    assert_eq!(literal, "-9223372036854775809");
    assert_eq!(magnitude, "9223372036854775809");
    assert_eq!(range, "signed_64");
    assert!(end > start, "诊断必须带真实 span，实际 {start}..{end}");
}

/// `-2^64` 连量级都超出 `u64`：失败发生在量级承载侧，仍是结构化诊断。
#[test]
fn negative_beyond_u64_magnitude_is_out_of_range() {
    let (literal, magnitude, range, start, end) = expect_out_of_range("micro probe() { let value = -18446744073709551616 }");
    assert_eq!(literal, "-18446744073709551616");
    assert_eq!(magnitude, "18446744073709551616");
    assert_eq!(range, "unsigned_64_magnitude");
    assert!(end > start, "诊断必须带真实 span，实际 {start}..{end}");
}
