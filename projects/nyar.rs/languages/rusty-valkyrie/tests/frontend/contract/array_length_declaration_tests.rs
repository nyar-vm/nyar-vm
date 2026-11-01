//! 数组长度不再是语言内建候选：没有声明候选时前端必须显式失败。
//!
//! 裸 `[T]` 的 `length(values)` 调用在没有声明候选时必须以 `HirError::UnresolvedCall`
//! 显式失败，前端不得按名字合成候选。

use rusty_valkyrie::{HirError, ValkyrieCompiler};

/// 数值原始类型名属于 `std/primitive` 的声明族；未声明时 canonical 以 CAN003 拒绝。
const PRIMITIVES: &str = "[primitive(\"std::primitive::usize\")] structure usize { } \
    [primitive(\"std::primitive::i32\")] structure i32 { } ";

/// 与 `asgard.router` 相同的调用形式：`length(segments)`。
const PROBE: &str = "micro probe(values: [i32]) -> usize { return length(values) }";

#[test]
fn undeclared_array_length_call_fails_without_frontend_synthesis() {
    let error = ValkyrieCompiler::default()
        .compile_source_to_program(&format!("{PRIMITIVES}{PROBE}"))
        .expect_err("缺少声明时数组长度调用不得成功");
    let rendered = format!("{error:?}");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "缺少声明必须结构化失败为 UnresolvedCall: {rendered}");
    assert!(rendered.contains("length"), "失败必须指出未解析的 length 调用: {rendered}");
}
