//! `[export(case: ...)]` 注解合同：取值只接受固定枚举，非法值在注解处结构化失败。
//!
//! 正例：`camelCase` / `snake_case` / 省略 `case` 都通过。
//! 反例：非法拼写与非字符串实参必须给出 `valkyrie.hir.invalid_export_case`，
//! 诊断 span 落在 `case` 实参上，并同时点名合法集合与当前非法值。

use rusty_valkyrie::{ValkyrieCompiler, compiler::diagnostics::pending};

fn compile(source: &str) -> Result<(), String> {
    pending::clear_pending_diagnostics();
    ValkyrieCompiler::default().compile_source(source).map(|_| ()).map_err(|error| error.to_string())
}

fn export_source(case_argument: &str) -> String {
    format!(
        r#"
namespace app;

[export({case_argument})]
micro two_sum() -> bool {{
    return true
}}
"#
    )
}

fn export_case_failure(case_argument: &str) -> (String, String) {
    let source = export_source(case_argument);
    let error = compile(&source).expect_err("非法 `case` 必须在注解处失败");
    let collected = pending::take_pending_diagnostics();
    let diagnostic = collected
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.row.code == "valkyrie.hir.invalid_export_case")
        .unwrap_or_else(|| panic!("必须产出 `valkyrie.hir.invalid_export_case` 结构化诊断: {collected:?}"));
    let rendered = format!("{:?}", diagnostic.row.args);
    assert!(rendered.contains("camelCase,snake_case"), "诊断必须点名合法取值集合: {rendered}");
    let span = diagnostic.row.primary_span.clone();
    let located = source.get(span.start..span.end).unwrap_or_default().to_string();
    (format!("{error}"), format!("{rendered}{located}"))
}

// ---------- 正例 ----------

#[test]
fn camel_case_export_is_accepted() {
    compile(&export_source(r#"case: "camelCase""#)).expect("`camelCase` 是合同允许的取值");
}

#[test]
fn snake_case_export_is_accepted() {
    compile(&export_source(r#"case: "snake_case""#)).expect("`snake_case` 是合同允许的取值");
}

#[test]
fn export_without_case_is_accepted() {
    compile(&export_source("name: \"twoSum\"")).expect("省略 `case` 时保留本地名");
}

// ---------- 反例 ----------

#[test]
fn illegal_case_spelling_fails_at_the_annotation() {
    let (error, collected) = export_case_failure(r#"case: "possibleBipartition""#);
    assert!(error.contains("possibleBipartition"), "错误必须点名当前非法值: {error}");
    assert!(collected.contains("possibleBipartition"), "诊断定位必须落在 `case` 实参上: {collected}");
    assert!(collected.contains("allowed"), "诊断必须携带合法取值集合参数: {collected}");
}

#[test]
fn non_string_case_argument_fails_at_the_annotation() {
    let (error, collected) = export_case_failure("case: 1");
    assert!(error.contains("<non-string literal>"), "非字符串实参必须以结构化占位点名: {error}");
    assert!(collected.contains("camelCase,snake_case"), "诊断必须点名合法取值集合: {collected}");
}
