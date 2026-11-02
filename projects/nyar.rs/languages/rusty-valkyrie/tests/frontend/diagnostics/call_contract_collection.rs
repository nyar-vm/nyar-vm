//! 块 3a：调用合同收集 —— 同一编译单元的全部未解析调用一次入汇。
//!
//! 正例：多个未解析调用在一次编译里全部收集、按源码顺序稳定渲染；
//! 反例（旧行为锚点）：返回的首个错误仍是遍历顺序的第一条，文本与旧格式逐字节一致；
//! 干净源码不在暂存汇留下任何残留。

use rusty_valkyrie::compiler::{
    diagnostics::{pending, report, report::DiagnosticRenderOptions},
    hir::lowering::ValkyrieCompiler,
};

use super::support::selectors;

#[test]
fn collects_every_unresolved_call_in_one_compilation_unit() {
    pending::clear_pending_diagnostics();
    let source = "micro main() -> void {\n    alpha()\n    beta()\n    gamma()\n}\n";
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("undeclared callees must fail");

    // 旧行为不变：返回的首个错误仍是遍历顺序上的第一条，文本与旧格式一致。
    let first = error.to_string();
    assert!(first.starts_with("unresolved call `alpha` in main at <hir_module>:"), "{first}");

    // 新行为：同一编译单元的全部未解析调用都已记入暂存汇。
    let collected = pending::take_pending_diagnostics();
    assert_eq!(collected.total(), 3, "all three unresolved calls must be collected: {collected:?}");
    assert!(collected.diagnostics.iter().all(|d| d.row.code == "valkyrie.hir.unresolved_call"));
    assert_eq!(selectors(&collected), vec!["alpha", "beta", "gamma"]);

    // 渲染后一次列出全部独立错误，且按 span 顺序稳定。
    let rendered = report::render_compile_diagnostics(
        &collected,
        &|id| (id == "<hir_module>").then(|| source.to_string()),
        &DiagnosticRenderOptions::default(),
    );
    let alpha_at = rendered.find("`alpha`").expect("rendered contains alpha");
    let beta_at = rendered.find("`beta`").expect("rendered contains beta");
    let gamma_at = rendered.find("`gamma`").expect("rendered contains gamma");
    assert!(alpha_at < beta_at && beta_at < gamma_at, "render order must follow source order: {rendered}");
    assert!(rendered.contains("--> <hir_module>:2:5"), "{rendered}");
    assert!(rendered.contains("--> <hir_module>:3:5"), "{rendered}");
    assert!(rendered.contains("--> <hir_module>:4:5"), "{rendered}");
    assert!(rendered.trim_end().ends_with("error: aborting due to 3 previous errors"), "{rendered}");
}

#[test]
fn clean_source_leaves_pending_sink_empty() {
    pending::clear_pending_diagnostics();
    let source = "micro main() -> void {\n}\n";
    ValkyrieCompiler::default().compile_source(source).expect("empty function compiles");
    assert!(pending::pending_diagnostics_is_empty());
}
