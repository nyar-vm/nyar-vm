//! 运算符调用把接收者类型作为其余实参的期望类型。
//!
//! `x == 0 - 1` 中嵌套的 `0 - 1` 没有自足宽度；只有接收者 `x: i64` 能定下
//! `infix ==` 与 `infix -` 的数值宽度。接收者类型不可推断时仍须 `unresolved_call`
//! 失败闭合，不得静默选一个宽度。

use rusty_valkyrie::ValkyrieCompiler;

const NUMERIC_DECL: &str = r#"
[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }

[host_contract]
private micro __i32_sub(lhs: i32, rhs: i32): i32

[host_contract]
private micro __i64_sub(lhs: i64, rhs: i64): i64

[host_contract]
private micro __i32_eq(lhs: i32, rhs: i32): bool

[host_contract]
private micro __i64_eq(lhs: i64, rhs: i64): bool

imply i32 {
    infix `-`(self, rhs: Self): Self { __i32_sub(self, rhs) }
    infix `==`(self, rhs: Self): bool { __i32_eq(self, rhs) }
}

imply i64 {
    infix `-`(self, rhs: Self): Self { __i64_sub(self, rhs) }
    infix `==`(self, rhs: Self): bool { __i64_eq(self, rhs) }
}
"#;

#[test]
fn operator_receiver_type_supplies_bare_literal_context() {
    let source = format!(
        r#"
namespace demo.operator_context;

{NUMERIC_DECL}

micro probe(x: i64) -> bool {{
    if x == 0 - 1 {{
        return true
    }}
    return false
}}
"#
    );
    ValkyrieCompiler::default()
        .compile_source(&source)
        .expect("接收者 i64 必须把 i64 上下文供给嵌套 `0 - 1`");
}

#[test]
fn operator_with_uninferable_receiver_fails_closed() {
    let source = format!(
        r#"
namespace demo.operator_context_unknown;

{NUMERIC_DECL}

micro probe<T>(x: T) -> bool {{
    return x == 0 - 1
}}
"#
    );
    let message = ValkyrieCompiler::default()
        .compile_source(&source)
        .expect_err("接收者类型不可推断时不得静默选数值宽度")
        .to_string();
    assert!(message.contains("unresolved call"), "必须保持 unresolved_call 失败闭合，实际: {message}");
}
