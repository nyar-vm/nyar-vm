//! 自由函数实参位的 nullary 变体构造上下文推断。
//!
//! `f(None)` 这类调用的实参没有自足类型；上下文必须由同名自由函数候选的形参
//! 类型逐槽供给（与成员调用 `member_call_argument_contexts` 同口径）。多候选
//! 在同一槽类型冲突时必须失败闭合，不得猜其中一个。

use rusty_valkyrie::ValkyrieCompiler;

const OPTION_DECL: &str = r#"
[primitive("std::primitive::i32")]
structure i32 { }

[tag(ProbeOptionKind)]
unite ProbeOption<T> {
    [tag(0)]
    [constructor]
    Some { value: T },
    [tag(1, default)]
    None,
}
"#;

#[test]
fn free_call_argument_none_resolves_from_parameter_type() {
    let source = format!(
        r#"
namespace demo.option_arg;

{OPTION_DECL}

micro takes_option(x: ProbeOption<i32>) -> i32 {{
    return 0
}}

micro main() -> i32 {{
    return takes_option(None)
}}
"#
    );
    ValkyrieCompiler::default()
        .compile_source(&source)
        .expect("裸 `None` 在自由函数实参位必须由形参类型反推 unite 身份");
}

#[test]
fn free_call_argument_none_with_conflicting_candidates_fails_closed() {
    let source = format!(
        r#"
namespace demo.option_arg_conflict;

{OPTION_DECL}

micro pick(x: ProbeOption<i32>) -> i32 {{
    return 1
}}

micro pick(x: i32) -> i32 {{
    return 2
}}

micro main() -> i32 {{
    return pick(None)
}}
"#
    );
    let message = ValkyrieCompiler::default().compile_source(&source).expect_err("同名候选形参类型冲突时不得猜测其一").to_string();
    assert!(message.contains("unresolved call"), "冲突必须保持 unresolved_call 失败闭合，实际: {message}");
}
