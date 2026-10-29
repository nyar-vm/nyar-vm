//! Legion API `plan()` 结构化载荷（非 CLI 文本）。

use crate::support::{
    create_browser_wasm_dual_host_prune_workspace, create_browser_wasm_host_random_workspace, create_smoke_project_with_build,
};
use rusty_valkyrie::legion::{CanonicalTarget, CompileOptions, PlanRequest, compile, plan};

#[test]
fn plan_and_compile_smoke_via_api() {
    let fixture = create_smoke_project_with_build(
        "legion-api-smoke",
        r#"{ target: "ovm" }"#,
        r#"[main]
micro main() -> i64 {
    return 0
}
"#,
    );

    let build_plan = plan(PlanRequest {
        project_dir: fixture.project_dir.clone(),
        target: CanonicalTarget::parse("ovm").expect("ovm"),
        output_dir: Some(fixture.project_dir.join("dist").join("api-out")),
    })
    .expect("plan");

    assert!(build_plan.is_compilable());
    assert!(!build_plan.source_bundle_hash.is_empty());
    assert_eq!(build_plan.compile_arch_key, "ovm");

    let report = compile(&build_plan, CompileOptions { verbose: false, debug_artifacts: false }).expect("compile");
    assert!(report.success, "compile failed: {:?}", report.diagnostics);
    assert!(report.driver.is_some());
    assert!(!report.artifacts.is_empty());
}

#[test]
fn plan_api_reports_js_random_host_provider_for_entry_closure() {
    let fixture = create_browser_wasm_host_random_workspace("legion-api-random");
    let build_plan =
        plan(PlanRequest { project_dir: fixture.project_dir.clone(), target: CanonicalTarget::browser(), output_dir: None }).expect("plan");

    assert_eq!(build_plan.selected_host_provider_count, 1);
    assert_eq!(build_plan.selected_host_providers[0].required_capabilities, vec!["js.random".to_string()]);
    assert_eq!(build_plan.selected_host_providers[0].foreign_bindings, vec![r#"js("random")"#.to_string()]);
    assert_eq!(build_plan.host_required_capabilities, vec!["js.random".to_string()]);
}

#[test]
fn compile_api_emits_single_pruned_host_binding_evidence() {
    let fixture = create_browser_wasm_dual_host_prune_workspace("legion-api-compile-prune");
    let output_dir = fixture.project_dir.join("dist").join("api-prune");
    let build_plan = plan(PlanRequest {
        project_dir: fixture.project_dir.clone(),
        target: CanonicalTarget::browser(),
        output_dir: Some(output_dir.clone()),
    })
    .expect("plan");
    assert_eq!(build_plan.selected_host_provider_count, 1);

    let report = compile(&build_plan, CompileOptions { verbose: false, debug_artifacts: false }).expect("compile");
    assert!(report.success, "compile failed: {:?}", report.diagnostics);
    let evidence = report.evidence.expect("compile evidence");
    assert_eq!(evidence.host_provider_bindings.len(), 1);
    assert!(evidence.host_provider_bindings[0].required_capabilities.contains(&"js.random".to_string()));
    assert_eq!(report.host_required_capabilities, vec!["js.random".to_string()]);
}
