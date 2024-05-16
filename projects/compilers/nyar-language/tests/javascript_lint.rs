use nyar_language::javascript::{JavascriptLintOptions, lint_javascript_source};

#[test]
fn fixes_loose_equality() {
    let (out, issues) = lint_javascript_source("if (a == b) {\n}\n", &JavascriptLintOptions { fix: true });
    assert!(out.contains("==="));
    assert_eq!(issues.len(), 1);
    assert!(issues[0].fixed);
}
