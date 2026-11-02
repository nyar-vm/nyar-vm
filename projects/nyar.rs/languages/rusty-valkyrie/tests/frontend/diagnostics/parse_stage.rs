//! 块 2：解析期结构化诊断进同一诊断汇。
//!
//! 已核实的 oak 行为（`oak.rs` a202114d）：Valkyrie 文法 `Result` 早退，实际产出
//! fatal 单错、`diagnostics` 恒为空——所以本组测试锁定的是"如实转述"：
//! 恰好一条结构化解析诊断，code/phase/owner/span 稳定，消息是文法干净文本而不是
//! Debug 转储；数量不伪造，oak 恢复记录接入后这里自动扩展。
//! 正例：语法错误产生一条解析诊断；反例：干净源码不产生解析诊断。

use nyar_types::{DiagnosticOwnerClass, DiagnosticPhase};
use rusty_valkyrie::compiler::{
    diagnostics::{pending, report, report::DiagnosticRenderOptions},
    hir::lowering::ValkyrieCompiler,
};

#[test]
fn syntax_error_is_recorded_as_single_structured_parse_diagnostic() {
    pending::clear_pending_diagnostics();
    let source = "micro main() -> i64 {\n    @@@\n    return 0\n}\n";
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("syntax error must fail");
    let first = error.to_string();
    assert!(first.contains("此处需要表达式"), "returned first error keeps the oak message: {first}");

    let collected = pending::take_pending_diagnostics();
    assert_eq!(collected.total(), 1, "oak records exactly one fatal parse error today: {collected:?}");
    let diagnostic = &collected.diagnostics[0];
    assert_eq!(diagnostic.row.code, "valkyrie.frontend.parse.syntax");
    assert!(matches!(diagnostic.row.phase, DiagnosticPhase::Parsing));
    assert!(matches!(diagnostic.row.owner.class, DiagnosticOwnerClass::Syntax));
    assert_eq!(diagnostic.row.owner.producer, "oak-valkyrie");
    assert_eq!(diagnostic.row.source.id, "<frontend>");
    assert!(diagnostic.row.primary_span.start > 0, "parse error must carry a byte offset: {diagnostic:?}");
    assert_eq!(diagnostic.row.primary_span.end, diagnostic.row.primary_span.start + 1);
    assert_eq!(diagnostic.message, "此处需要表达式", "clean grammar message, not a debug dump");

    // 无源码原文时退化为字节区间定位；code 与页脚稳定。
    let rendered = report::render_compile_diagnostics(&collected, &|_| None, &DiagnosticRenderOptions::default());
    assert!(rendered.contains("error[valkyrie.frontend.parse.syntax]: 此处需要表达式"), "{rendered}");
    assert!(rendered.contains("--> <frontend>:"), "{rendered}");
    assert!(rendered.trim_end().ends_with("error: aborting due to 1 previous error"), "{rendered}");
}

#[test]
fn clean_source_records_no_parse_diagnostic() {
    pending::clear_pending_diagnostics();
    let source = "micro main() -> i64 {\n    return 0\n}\n";
    ValkyrieCompiler::default().compile_source(source).expect("clean source compiles");
    assert!(pending::pending_diagnostics_is_empty());
}
