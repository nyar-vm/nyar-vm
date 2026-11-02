//! 诊断测试共享夹具。

use nyar_types::{DiagnosticArg, DiagnosticOwner, DiagnosticPhase, DiagnosticRow, DiagnosticSeverity, DiagnosticSource, DiagnosticSpan};
use rusty_valkyrie::compiler::diagnostics::{
    pending::{CompileDiagnostic, PendingDiagnostics},
    report::{self, DiagnosticRenderOptions},
};

pub(super) fn diagnostic(source: &str, start: usize, end: usize, code: &str, message: &str, selector: Option<&str>) -> CompileDiagnostic {
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

pub(super) fn pending_of(diagnostics: Vec<CompileDiagnostic>) -> PendingDiagnostics {
    PendingDiagnostics { diagnostics, overflow: 0 }
}

pub(super) fn render(pending: &PendingDiagnostics, source: &str) -> String {
    report::render_compile_diagnostics(pending, &|id| (id == "main.v").then(|| source.to_string()), &DiagnosticRenderOptions::default())
}

pub(super) fn selectors(pending: &PendingDiagnostics) -> Vec<String> {
    pending
        .diagnostics
        .iter()
        .map(|diagnostic| match diagnostic.row.args.get("selector") {
            Some(DiagnosticArg::String(selector)) => selector.clone(),
            other => panic!("expected selector arg, got {other:?}"),
        })
        .collect()
}
