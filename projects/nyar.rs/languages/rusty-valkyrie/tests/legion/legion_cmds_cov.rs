//! `legion cov` 集成测试。

use std::{fs, process::ExitCode};

use rusty_valkyrie::legion::cmds::cov::{CovArgs, infer_coverage_features, run};

use crate::support::create_smoke_project_with_manifest;

/// 覆盖率报告依赖 `valkyrie.v` 的 asgard.plotter 资产；未配置 `VALKYRIE_V_ROOT` 时跳过。
fn valkyrie_v_root_configured() -> bool {
    match std::env::var_os("VALKYRIE_V_ROOT") {
        Some(root) if !root.is_empty() => true,
        _ => {
            eprintln!("skip: VALKYRIE_V_ROOT 未配置，覆盖率报告需要 valkyrie.v 资产");
            false
        }
    }
}

#[test]
fn coverage_generates_json_and_html() {
    if !valkyrie_v_root_configured() {
        return;
    }
    let fixture = create_smoke_project_with_manifest(
        "legion-cov",
        r#"{
    name: "cov_app",
    build: [
        { target: "ovm" }
    ]
}
"#,
        r#"micro main() -> i64 {
    return 0;
}
"#,
    );

    let test_dir = fixture.project_dir.join("test");
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(
        test_dir.join("suite.v"),
        r#"[test]
micro add_two() -> unit {
}

[benchmark]
micro fib() -> unit {
}
"#,
    )
    .unwrap();

    let status = run(&CovArgs { project_dir: fixture.project_dir.clone(), verbose: true, standalone: false }).unwrap();
    assert_eq!(status, ExitCode::SUCCESS);

    let json_path = fixture.project_dir.join(".cache/converge/coverage.json");
    let html_path = fixture.project_dir.join("dist/legion-converge/index.html");
    assert!(json_path.is_file(), "missing {}", json_path.display());
    assert!(html_path.is_file(), "missing {}", html_path.display());

    let json = fs::read_to_string(&json_path).unwrap();
    assert!(json.contains("\"covered\""));
    assert!(json.contains("micro"));
    assert!(json.contains("test"));

    let html = fs::read_to_string(&html_path).unwrap();
    assert!(html.contains("Coverage Report"));
    assert!(html.contains("micro 函数"));
    assert!(!html.contains("asgard-runtime.js"));
}

#[test]
fn infer_detects_core_features() {
    let fixture = create_smoke_project_with_manifest(
        "legion-cov-infer",
        r#"{
    name: "infer_app",
    build: [ { target: "ovm" } ]
}
"#,
        r#"micro main() -> i64 {
    let x: f64 = 1.0
    match x {
        _ => 0
    }
}
"#,
    );
    let features = infer_coverage_features(&fixture.project_dir);
    assert!(features.iter().any(|f| f == "micro"));
    assert!(features.iter().any(|f| f == "match"));
    assert!(features.iter().any(|f| f == "float"));
}
