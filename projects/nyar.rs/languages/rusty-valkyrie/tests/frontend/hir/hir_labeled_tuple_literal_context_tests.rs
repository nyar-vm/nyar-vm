//! 带标签元组的字面量精化只对元组字面量实参生效。
//!
//! `Option<(ordinal: i32, value: i64)>` 这类返回类型要求 `Some { value: (ordinal, value) }`
//! 的载荷按形式字段取得标签；元组字面量本身没有标签语法，标签只能由期望类型提供。
//! 精化必须严格限定：位置类型不等或期望类型不是 `LabeledTuple` 时一律失败闭合，
//! 不得把 `Tuple` 与 `LabeledTuple` 全局视为兼容而擦掉标签。

use rusty_valkyrie::ValkyrieCompiler;

const LABELED_TUPLE_DECL: &str = r#"
[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }

unite Option<T> {
    Some { value: T }
    None {}
}
"#;

#[test]
fn variant_payload_tuple_literal_inherits_return_type_labels() {
    let body = r#"
namespace demo.labeled_tuple_literal;

micro wrap(ordinal: i32, value: i64) -> Option<(ordinal: i32, value: i64)> {
    return Some { value: (ordinal, value) }
}
"#;
    let source = [LABELED_TUPLE_DECL, body].join("\n");
    ValkyrieCompiler::default().compile_source_to_program(&source).expect("变体载荷的元组字面量必须按返回类型的字段标签精化");
}

#[test]
fn variant_payload_tuple_literal_with_mismatched_positions_fails() {
    let body = r#"
namespace demo.labeled_tuple_literal_mismatch;

micro wrap(ordinal: i32, value: i64) -> Option<(ordinal: i64, value: i64)> {
    return Some { value: (ordinal, value) }
}
"#;
    let source = [LABELED_TUPLE_DECL, body].join("\n");
    let message = ValkyrieCompiler::default().compile_source(&source).expect_err("位置类型不等时不得静默按标签改写").to_string();
    assert!(message.contains("is incompatible with the function return type"), "位置类型不等必须保持返回类型不兼容失败，实际: {message}");
}

#[test]
fn tuple_literal_is_not_elaborated_for_unlabeled_formal() {
    let body = r#"
namespace demo.tuple_literal_unlabeled_formal;

micro take(pair: (i64, i64)) -> i64 {
    return 0
}

micro call(x: i32, y: i32) -> i64 {
    return take((x, y))
}
"#;
    let source = [LABELED_TUPLE_DECL, body].join("\n");
    let message = ValkyrieCompiler::default().compile_source(&source).expect_err("形式参数不是带标签元组时不得精化").to_string();
    assert!(message.contains("unresolved call"), "必须保持 unresolved_call 失败闭合，实际: {message}");
}
