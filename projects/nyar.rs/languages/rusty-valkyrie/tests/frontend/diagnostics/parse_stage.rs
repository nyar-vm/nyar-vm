//! 块 2：解析期结构化诊断进同一诊断汇。
//!
//! oak 根循环恢复（`oak.rs` `1f766c07`）接入后，item 级语法错误不再 fatal：
//! 错误记入诊断汇、坏 item 由 `Error` 节点跳过、兄弟 item 照常进入编译。
//! 本组测试锁定"如实转述"：诊断条数与位置完全来自 oak（恢复重试产生的重复失败
//! 也如实记录，不凑整不伪造），code/phase/owner/span 稳定，消息是文法干净文本而不是
//! Debug 转储。
//! 正例：坏源码产生等量解析诊断且编译在恢复模块上继续；反例：干净源码不产生解析诊断。

use nyar_types::{DiagnosticOwnerClass, DiagnosticPhase};
use rusty_valkyrie::compiler::{
    diagnostics::{pending, report, report::DiagnosticRenderOptions},
    hir::lowering::ValkyrieCompiler,
};

#[test]
fn syntax_error_is_recorded_as_structured_parse_diagnostic() {
    pending::clear_pending_diagnostics();
    let source = "micro main() -> i64 {\n    @@@\n    return 0\n}\n";
    // 根循环恢复后 item 级语法错误不再致命：坏 item 被丢弃，编译在恢复模块上继续。
    ValkyrieCompiler::default().compile_source(source).expect("recovery must keep the pipeline alive");

    let collected = pending::take_pending_diagnostics();
    // `@@@` 触发失败后根循环在 `@` 边界重试，第二次失败同样如实记录：两条诊断 @[26, 43]。
    assert_eq!(collected.total(), 2, "oak records each failed parse attempt: {collected:?}");
    let spans: Vec<(usize, usize)> = collected.diagnostics.iter().map(|diagnostic| (diagnostic.row.primary_span.start, diagnostic.row.primary_span.end)).collect();
    assert_eq!(spans, [(26, 27), (43, 44)], "parse error byte offsets: {collected:?}");
    for diagnostic in &collected.diagnostics {
        assert_eq!(diagnostic.row.code, "valkyrie.frontend.parse.syntax");
        assert!(matches!(diagnostic.row.phase, DiagnosticPhase::Parsing));
        assert!(matches!(diagnostic.row.owner.class, DiagnosticOwnerClass::Syntax));
        assert_eq!(diagnostic.row.owner.producer, "oak-valkyrie");
        assert_eq!(diagnostic.row.source.id, "<frontend>");
        assert_eq!(diagnostic.message, "此处需要表达式", "clean grammar message, not a debug dump");
    }

    // 无源码原文时退化为字节区间定位；code 与页脚稳定。
    let rendered = report::render_compile_diagnostics(&collected, &|_| None, &DiagnosticRenderOptions::default());
    assert!(rendered.contains("error[valkyrie.frontend.parse.syntax]: 此处需要表达式"), "{rendered}");
    assert!(rendered.contains("--> <frontend>:"), "{rendered}");
    assert!(rendered.trim_end().ends_with("error: aborting due to 2 previous errors"), "{rendered}");
}

#[test]
fn multiple_broken_items_record_multiple_parse_diagnostics() {
    pending::clear_pending_diagnostics();
    let source = "micro a() { + }\nclass B {\n  micro c() { [1, 2 }\nmicro d() {}\n";
    let module = ValkyrieCompiler::default().compile_source(source).expect("recovery must keep the pipeline alive");

    let collected = pending::take_pending_diagnostics();
    assert_eq!(collected.total(), 2, "each broken item records exactly one parse diagnostic: {collected:?}");
    assert_eq!(collected.diagnostics[0].row.primary_span.start, 12, "first broken item offset: {collected:?}");
    assert_eq!(collected.diagnostics[1].row.primary_span.start, 46, "second broken item offset: {collected:?}");
    for diagnostic in &collected.diagnostics {
        assert_eq!(diagnostic.row.code, "valkyrie.frontend.parse.syntax");
    }
    assert!(module.functions.iter().any(|function| function.name.as_str() == "d"), "兄弟 item 必须保留: {:?}", module.functions);
}

#[test]
fn broken_expression_records_diagnostic_and_keeps_sibling_item() {
    pending::clear_pending_diagnostics();
    let source = "micro a() { + }\nmicro b() {}\n";
    let module = ValkyrieCompiler::default().compile_source(source).expect("recovery must keep the pipeline alive");

    let collected = pending::take_pending_diagnostics();
    assert_eq!(collected.total(), 1, "broken expression records exactly one parse diagnostic: {collected:?}");
    assert_eq!(collected.diagnostics[0].row.primary_span.start, 12, "broken expression offset: {collected:?}");
    assert!(module.functions.iter().any(|function| function.name.as_str() == "b"), "兄弟 item 必须保留: {:?}", module.functions);
    assert!(!module.functions.iter().any(|function| function.name.as_str() == "a"), "坏 item 不得进入 HIR: {:?}", module.functions);
}

#[test]
fn unterminated_item_with_garbage_records_diagnostic_and_recovers() {
    pending::clear_pending_diagnostics();
    let source = "micro a() { [1, 2\n$$$\nmicro b() {}\n";
    let module = ValkyrieCompiler::default().compile_source(source).expect("recovery must keep the pipeline alive");

    let collected = pending::take_pending_diagnostics();
    assert_eq!(collected.total(), 1, "unterminated item records exactly one parse diagnostic: {collected:?}");
    assert_eq!(collected.diagnostics[0].row.primary_span.start, 18, "unclosed array reports at the garbage offset: {collected:?}");
    assert!(module.functions.iter().any(|function| function.name.as_str() == "b"), "兄弟 item 必须保留: {:?}", module.functions);
}

#[test]
fn clean_source_records_no_parse_diagnostic() {
    pending::clear_pending_diagnostics();
    let source = "micro main() -> i64 {\n    return 0\n}\n";
    ValkyrieCompiler::default().compile_source(source).expect("clean source compiles");
    assert!(pending::pending_diagnostics_is_empty());
}
