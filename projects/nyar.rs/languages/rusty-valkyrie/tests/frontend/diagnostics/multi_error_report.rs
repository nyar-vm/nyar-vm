//! 诊断模型（块 1）测试：暂存汇存取与确定性渲染。
//!
//! 覆盖：排序、行列定位、`^` 指向、无源码退化、渲染确定性、暂存上限丢弃计数。

use nyar_types::{DiagnosticArg, DiagnosticOwner, DiagnosticPhase, DiagnosticRow, DiagnosticSeverity, DiagnosticSource, DiagnosticSpan};
use rusty_valkyrie::compiler::diagnostics::{
    pending::{self, CompileDiagnostic, PendingDiagnostics},
    report::{self, DiagnosticRenderOptions},
};

fn diagnostic(source: &str, start: usize, end: usize, code: &str, message: &str, selector: Option<&str>) -> CompileDiagnostic {
    let mut row = DiagnosticRow::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticPhase::Hir,
        DiagnosticOwner::frontend("rusty-valkyrie"),
        DiagnosticSource { id: source.into(), revision: None },
        DiagnosticSpan { start, end },
    );
    if let Some(selector) = selector {
        row = row.with_arg("selector", DiagnosticArg::String(selector.into()));
    }
    CompileDiagnostic::new(row, message)
}

fn pending_of(diagnostics: Vec<CompileDiagnostic>) -> PendingDiagnostics {
    PendingDiagnostics { diagnostics, overflow: 0 }
}

fn render(pending: &PendingDiagnostics, source: &str) -> String {
    report::render_compile_diagnostics(pending, &|id| (id == "main.v").then(|| source.to_string()), &DiagnosticRenderOptions::default())
}

#[test]
fn renders_line_column_and_caret_for_located_span() {
    let source = "micro main() -> void {\n    let value = left + right\n}\n";
    // `left + right` 位于第二行；列号与指向宽度按实际字节偏移计算。
    let start = source.find("left").expect("fixture contains call site");
    let end = source.find("right").expect("fixture contains call site") + "right".len();
    let pending = pending_of(vec![diagnostic(
        "main.v",
        start,
        end,
        "valkyrie.hir.unresolved_call",
        "unresolved call `infix +` in main",
        Some("infix +"),
    )]);
    let rendered = render(&pending, source);

    assert!(rendered.contains("error[valkyrie.hir.unresolved_call]: unresolved call `infix +` in main"), "{rendered}");
    assert!(rendered.contains("--> main.v:2:17"), "{rendered}");
    assert!(rendered.contains("2 |     let value = left + right"), "{rendered}");
    assert!(rendered.contains("^".repeat("left + right".len()).as_str()), "{rendered}");
    assert!(rendered.trim_end().ends_with("error: aborting due to 1 previous error"), "{rendered}");
}

#[test]
fn sorts_by_file_then_span_start_for_stable_output() {
    let source = "alpha\nbeta\ngamma\n";
    let pending = pending_of(vec![
        diagnostic("b.v", 6, 10, "valkyrie.hir.unresolved_call", "unresolved call `infix *` in b", Some("infix *")),
        diagnostic("a.v", 6, 9, "valkyrie.hir.unresolved_call", "unresolved call `infix <` in main", Some("infix <")),
        diagnostic("a.v", 0, 5, "valkyrie.hir.unresolved_call", "unresolved call `infix +` in main", Some("infix +")),
    ]);
    let rendered = render(&pending, source);
    let first_plus = rendered.find("infix +").expect("rendered contains +");
    let first_less = rendered.find("infix <").expect("rendered contains <");
    let first_star = rendered.find("infix *").expect("rendered contains *");
    assert!(first_plus < first_less, "same file must be ordered by span start: {rendered}");
    assert!(first_less < first_star, "files must be ordered lexicographically: {rendered}");

    let rendered_twice = render(&pending, source);
    assert_eq!(rendered, rendered_twice, "rendering must be deterministic");
}

#[test]
fn falls_back_to_byte_span_when_source_text_is_unavailable() {
    let pending = pending_of(vec![diagnostic(
        "missing.v",
        1195,
        1231,
        "valkyrie.hir.unresolved_call",
        "unresolved call `infix ==` in data_cache_matches",
        Some("infix =="),
    )]);
    let rendered = render(&pending, "irrelevant");
    assert!(rendered.contains("--> missing.v:1195..1231"), "{rendered}");
    assert!(!rendered.contains('|'), "no snippet without source text: {rendered}");
}

#[test]
fn reports_truncated_count_instead_of_silent_cut() {
    let diagnostics = (0..5)
        .map(|index| {
            diagnostic(
                "main.v",
                index,
                index + 1,
                "valkyrie.hir.unresolved_call",
                &format!("unresolved call `callee_{index}` in main"),
                Some(&format!("callee_{index}")),
            )
        })
        .collect::<Vec<_>>();
    let pending = pending_of(diagnostics);
    let options = DiagnosticRenderOptions { max_diagnostics: 3 };
    let rendered = report::render_compile_diagnostics(&pending, &|_| None, &options);
    assert_eq!(rendered.matches("error[valkyrie.hir.unresolved_call]").count(), 3, "{rendered}");
    assert!(rendered.contains("note: 2 more diagnostic(s) not shown (limit 3 reached)"), "{rendered}");
    assert!(rendered.trim_end().ends_with("error: aborting due to 5 previous errors"), "{rendered}");
}

#[test]
fn pending_sink_push_take_and_overflow_accounting() {
    pending::clear_pending_diagnostics();
    assert!(pending::pending_diagnostics_is_empty());

    let total = pending::PENDING_DIAGNOSTIC_LIMIT + 3;
    for index in 0..total {
        pending::push_compile_diagnostic(diagnostic("main.v", index, index, "valkyrie.hir.unresolved_call", "probe", None));
    }
    assert!(!pending::pending_diagnostics_is_empty());

    let taken = pending::take_pending_diagnostics();
    assert_eq!(taken.diagnostics.len(), pending::PENDING_DIAGNOSTIC_LIMIT);
    assert_eq!(taken.overflow, 3);
    assert_eq!(taken.total(), total);
    assert!(pending::pending_diagnostics_is_empty(), "take must drain the sink");
}

#[test]
fn empty_pending_renders_empty_string() {
    let rendered = report::render_compile_diagnostics(&PendingDiagnostics::default(), &|_| None, &DiagnosticRenderOptions::default());
    assert!(rendered.is_empty());
}
