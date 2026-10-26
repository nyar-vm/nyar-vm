//! Legion API `plan()` 结构化载荷（非 CLI 文本）。

use crate::support::create_smoke_project_with_build;
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
