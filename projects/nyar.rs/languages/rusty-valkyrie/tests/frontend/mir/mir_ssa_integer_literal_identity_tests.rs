//! 整数字面量形参判定只认已冻结的 primitive 身份，不再按短名认数值。
//!
//! 三个反例（修前均编译零诊断）：
//! 1) 名叫 `i32` 的普通 class 当形参 → 旧 `TypeRelation::is_numeric_type` 按短名判数值，字面量被接受；
//! 2) 缺指针宽度别名的 `Named("usize")` 形参 → 旧 `is_integer_literal_formal_type` 按名字接受；
//! 3) 本地声明表缺失的无参 `Nominal` → 旧实现无条件 `true`（跨包缺表兜底）。
//!
//! 修后只允许声明携带 `[primitive("std::primitive::…")]` 身份的数值形参接受整数字面量；
//! 其余一律让 `SMIR007`/overload 失败正常触发，不按名字猜。

use rusty_valkyrie::{MirLowerer, ValkyrieCompiler};

fn compiles_clean(source: &str) -> bool {
    match ValkyrieCompiler::default().compile_source(source) {
        Ok(hir) => MirLowerer::lower_module_semantic(&hir).diagnostics.is_empty(),
        Err(_) => false,
    }
}

fn mir_diagnostic_messages(source: &str) -> Vec<String> {
    let hir = ValkyrieCompiler::default().compile_source(source).expect("源码必须先完成 HIR lowering");
    MirLowerer::lower_module_semantic(&hir).diagnostics.iter().map(|diagnostic| format!("{diagnostic:?}")).collect()
}

/// `u64` 上下文的字面量保留完整 64 位正半域（`u64::MAX` 不再是 `signed_64` 范围错误）。
#[test]
fn u64_max_literal_settles_inside_unsigned_64_context() {
    let source = r#"
namespace std::primitive;
[primitive("std::primitive::u64")] structure u64 { }

micro probe(): unit {
    let value: std::primitive::u64 = 18446744073709551615
}
"#;
    assert!(compiles_clean(source), "u64 上下文必须承载 u64::MAX：{:?}", mir_diagnostic_messages(source));
}

/// `i64` 上下文装不下 `u64::MAX`：结算处给出结构化范围诊断，不静默截断。
#[test]
fn i64_context_rejects_u64_max_literal() {
    let source = r#"
namespace std::primitive;
[primitive("std::primitive::i64")] structure i64 { }

micro probe(): unit {
    let value: std::primitive::i64 = 18446744073709551615
}
"#;
    let messages = mir_diagnostic_messages(source);
    assert!(messages.iter().any(|message| message.contains("out of range")), "i64 上下文必须报告范围诊断，实际 {messages:?}");
}

/// 无注解时不回落到 `i64` 截断：默认宽度取能容纳该值的最小有符号宽度，否则 `u64`。
#[test]
fn context_free_u64_max_literal_defaults_to_unsigned_64() {
    let source = r#"
micro probe(): unit {
    let value = 18446744073709551615
}
"#;
    assert!(compiles_clean(source), "无注解的 u64::MAX 必须默认 u64 而非截断：{:?}", mir_diagnostic_messages(source));
}

/// 反例 1：`class i32` 没有 primitive 身份，整数字面量不得按名字被当成数值实参。
#[test]
fn class_named_i32_is_not_a_numeric_formal() {
    let source = r#"
class i32 {
}

micro take(x: i32) -> i32 {
    return x
}

micro call() -> i32 {
    return take(7)
}
"#;
    assert!(!compiles_clean(source), "名叫 `i32` 的普通 class 不得按短名接受整数字面量");
}

/// 反例 2：缺别名的 `Named("usize")` 形参不得按名字接受整数字面量。
#[test]
fn unnamed_platform_width_formal_rejects_integer_literal() {
    let source = r#"
micro take(x: usize) -> usize {
    return x
}

micro call() -> usize {
    return take(7)
}
"#;
    assert!(!compiles_clean(source), "未结算别名的 `usize` 形参不得按名字猜位宽并接受字面量");
}

/// 正向：携带 `[primitive]` 身份的同名声明仍按身份接受整数字面量。
#[test]
fn primitive_identity_formal_still_accepts_integer_literal() {
    let source = r#"
namespace std::primitive;
[primitive("std::primitive::i32")] structure i32 { }

micro take(x: std::primitive::i32) -> std::primitive::i32 {
    return x
}

micro call() -> std::primitive::i32 {
    return take(7)
}
"#;
    assert!(compiles_clean(source), "`[primitive]` 身份是合法数值形参: 必须零诊断");
}
