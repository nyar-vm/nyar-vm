//! 注释语法合同探针：`#` 行注释、`<# #>` 块注释、`#?` / `⍝` 文档注释。
//!
//! 正例：三种形态在文件首行、表达式后、参数列表内等位置都被词法吞为 trivia，
//! 注释内的引号 / 反斜杠 / `#` / `<#` / `#>` 不干扰后续字面量与语句捕获。
//! 反例：`//` 与 `/* */` 不是 Valkyrie 注释，必须落到结构化解析诊断。

use rusty_valkyrie::compiler::{diagnostics::pending, hir::lowering::ValkyrieCompiler};

use crate::primitives::SourceFixtureCompile;

/// 编译并断言零诊断；失败时把诊断打进 panic 消息便于定位。
fn assert_clean(source: &str) {
    pending::clear_pending_diagnostics();
    ValkyrieCompiler::default().compile_fixture_source(source).expect("source must compile");
    let collected = pending::take_pending_diagnostics();
    assert!(collected.total() == 0, "expected zero diagnostics, got: {collected:?}");
}

/// 编译并断言至少产生一条解析期语法诊断。
fn assert_parse_error(source: &str) {
    pending::clear_pending_diagnostics();
    let _ = ValkyrieCompiler::default().compile_source(source);
    let collected = pending::take_pending_diagnostics();
    assert!(
        collected.diagnostics.iter().any(|diagnostic| diagnostic.row.code == "valkyrie.frontend.parse.syntax"),
        "expected a parse syntax diagnostic, got: {collected:?}"
    );
}

#[test]
fn line_comment_on_first_line_of_file() {
    assert_clean("# file header\nmicro main() -> i64 {\n    return 0\n}\n");
}

#[test]
fn line_comment_after_expression() {
    assert_clean("micro main() -> i64 {\n    return 0 # trailing note\n}\n");
}

#[test]
fn line_comment_inside_parameter_list() {
    assert_clean("micro add(a: i64, # left operand\n b: i64) -> i64 {\n    return a\n}\n");
}

#[test]
fn line_comment_does_not_swallow_quote_or_backslash() {
    // 注释里的未闭合引号与反斜杠不得开启字符串捕获，下一行照常编译。
    assert_clean("micro main() -> i64 {\n    # path C:\\new and a \" quote\n    return 0\n}\n");
}

#[test]
fn line_comment_swallows_block_markers() {
    // `<#` 与 `#>` 出现在行注释内只是普通字符，不得触发块注释吞行。
    assert_clean("micro main() -> i64 {\n    # <# not a block #>\n    return 0\n}\n");
}

#[test]
fn block_comment_spans_multiple_lines() {
    assert_clean("micro main() -> i64 {\n    <# first\n       second\n       third #>\n    return 0\n}\n");
}

#[test]
fn block_comment_nests() {
    assert_clean("micro main() -> i64 {\n    <# outer <# inner #> still outer #>\n    return 0\n}\n");
}

#[test]
fn block_comment_between_tokens() {
    assert_clean("micro add(a: i64, <# between params #> b: i64) -> i64 {\n    return a\n}\n");
}

#[test]
fn block_comment_does_not_swallow_hash_or_quotes() {
    // 块注释内的 `#`、`'`、`"` 都是内容字符；闭标记后代码照常编译。
    assert_clean("micro main() -> i64 {\n    <# a # b ' c \" d #>\n    return 0\n}\n");
}

#[test]
fn doc_comment_apl_lamp_before_declaration() {
    assert_clean("⍝ documentation for main\nmicro main() -> i64 {\n    return 0\n}\n");
}

#[test]
fn doc_comment_hash_question_before_declaration() {
    assert_clean("#? documentation for main\nmicro main() -> i64 {\n    return 0\n}\n");
}

#[test]
fn double_slash_is_not_a_comment() {
    assert_parse_error("micro main() -> i64 {\n    // not a comment\n    return 0\n}\n");
}

#[test]
fn c_block_is_not_a_comment() {
    assert_parse_error("micro main() -> i64 {\n    /* not a comment */\n    return 0\n}\n");
}
