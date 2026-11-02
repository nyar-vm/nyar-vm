//! 块 4：去级联 —— 同一根因归并成一条并附受影响站点，上限截断有显式汇总。
//!
//! 正例：同 selector 多处调用点归并为一条 + 站点清单；归并组计入展示上限；
//! 反例：不同根因（不同 selector）不得归并；同一输入两次渲染输出完全一致。

use rusty_valkyrie::compiler::{
    diagnostics::{pending, report, report::DiagnosticRenderOptions},
    hir::lowering::ValkyrieCompiler,
};

use super::support::{diagnostic, pending_of, render, selectors};

#[test]
fn merges_same_root_cause_into_one_entry_with_site_count() {
    let source = "micro main() -> void {\n    alpha()\n    beta()\n    alpha()\n}\n";
    pending::clear_pending_diagnostics();
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("undeclared callees must fail");
    assert!(error.to_string().starts_with("unresolved call `alpha` in main at <hir_module>:"), "first error keeps walk order: {error}");

    let collected = pending::take_pending_diagnostics();
    // 暂存汇保留全部三条独立诊断（结构化消费者要全集）；归并只发生在渲染层。
    assert_eq!(collected.total(), 3);
    assert_eq!(selectors(&collected), vec!["alpha", "beta", "alpha"]);

    let rendered = report::render_compile_diagnostics(
        &collected,
        &|id| (id == "<hir_module>").then(|| source.to_string()),
        &DiagnosticRenderOptions::default(),
    );
    assert_eq!(rendered.matches("error[valkyrie.hir.unresolved_call]").count(), 2, "alpha 两次出现必须归并成一条: {rendered}");
    assert!(rendered.contains("= 1 more site with the same root cause: <hir_module>:4:5"), "{rendered}");
    assert!(rendered.trim_end().ends_with("error: aborting due to 3 previous errors"), "{rendered}");
}

#[test]
fn distinct_root_causes_are_not_merged() {
    let diagnostics = vec![
        diagnostic("a.v", 0, 5, "valkyrie.hir.unresolved_call", "unresolved call `alpha` in main", Some("alpha")),
        diagnostic("a.v", 6, 10, "valkyrie.hir.unresolved_call", "unresolved call `beta` in main", Some("beta")),
    ];
    let pending = pending_of(diagnostics);
    let rendered = render(&pending, "alpha\nbeta\n");
    assert_eq!(rendered.matches("error[valkyrie.hir.unresolved_call]").count(), 2, "不同根因不得归并: {rendered}");
    assert!(!rendered.contains("same root cause"), "{rendered}");
}

#[test]
fn cap_counts_merged_groups_and_reports_hidden_entries() {
    let mut diagnostics = Vec::new();
    // 4 个不同根因 + 1 个重复根因（归并后 4 组）；上限 3 → 隐藏 1 条。
    for index in 0..4 {
        diagnostics.push(diagnostic(
            "main.v",
            index * 8,
            index * 8 + 4,
            "valkyrie.hir.unresolved_call",
            &format!("unresolved call `callee_{index}` in main"),
            Some(&format!("callee_{index}")),
        ));
    }
    diagnostics.push(diagnostic("main.v", 64, 70, "valkyrie.hir.unresolved_call", "unresolved call `callee_0` in main", Some("callee_0")));
    let pending = pending_of(diagnostics);
    let options = DiagnosticRenderOptions { max_diagnostics: 3 };
    let rendered = report::render_compile_diagnostics(&pending, &|_| None, &options);
    assert_eq!(rendered.matches("error[valkyrie.hir.unresolved_call]").count(), 3, "{rendered}");
    assert!(rendered.contains("note: 1 more diagnostic(s) not shown (limit 3 reached)"), "{rendered}");
    assert!(rendered.trim_end().ends_with("error: aborting due to 5 previous errors"), "{rendered}");
}

#[test]
fn merge_output_is_deterministic_across_renders() {
    let diagnostics = vec![
        diagnostic("a.v", 11, 16, "valkyrie.hir.unresolved_call", "unresolved call `alpha` in main", Some("alpha")),
        diagnostic("a.v", 0, 5, "valkyrie.hir.unresolved_call", "unresolved call `alpha` in main", Some("alpha")),
    ];
    let pending = pending_of(diagnostics);
    let source = "alpha\nbeta\nalpha\n";
    let resolve = |id: &str| (id == "a.v").then(|| source.to_string());
    let first = report::render_compile_diagnostics(&pending, &resolve, &DiagnosticRenderOptions::default());
    let second = report::render_compile_diagnostics(&pending, &resolve, &DiagnosticRenderOptions::default());
    assert_eq!(first, second);
    // 排序后第一条是 span 起点 0 的站点，归并备注指向第二处。
    let anchor = first.find("--> a.v:1:1").expect("sorted by span start");
    let note = first.find("= 1 more site").expect("merged note");
    assert!(anchor < note, "{first}");
    assert!(first.contains("a.v:3:1"), "{first}");
}
