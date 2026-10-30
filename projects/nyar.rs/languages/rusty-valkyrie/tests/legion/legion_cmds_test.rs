//! `legion test` 集成测试。

use std::{fs, path::Path, process::ExitCode};

use rusty_valkyrie::legion::{
    cmds::{
        report::{legion_report_project_dir, report_roots},
        test::{TestArgs, run},
        test_engine::{discover_project_tests, parse_target_label, resolve_test_targets},
    },
    workspace::planner::{BuildRequest, WorkspaceResolver},
};

use crate::support::create_smoke_project_with_manifest;

/// HTML 报告依赖 `valkyrie.v` 的 asgard.plotter 资产；未配置 `VALKYRIE_V_ROOT` 时跳过，
/// 与 `tests/legion_cmds_report.rs`、`tests/legion_cmds_cov.rs` 保持同一约定。
fn valkyrie_v_root_configured() -> bool {
    match std::env::var_os("VALKYRIE_V_ROOT") {
        Some(root) if !root.is_empty() => true,
        _ => {
            eprintln!("skip: VALKYRIE_V_ROOT 未配置，测试报告需要 valkyrie.v 资产");
            false
        }
    }
}

fn write_test_suite(project_dir: &std::path::Path) {
    let test_dir = project_dir.join("test");
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(
        test_dir.join("suite.v"),
        r#"[test]
micro add_two() -> i64 {
    return 2
}

[test]
micro always_pass() -> i64 {
    return 0
}

[benchmark]
micro fib_bench() -> i64 {
    return 1
}
"#,
    )
    .unwrap();
}

#[test]
fn discover_finds_test_and_benchmark() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-discover",
        r#"{
    name: "test_app",
    build: [ { target: "ovm" } ]
}
"#,
        r#"micro main() -> i64 {
    return 0
}
"#,
    );
    write_test_suite(&fixture.project_dir);
    let functions = discover_project_tests(&fixture.project_dir);
    assert_eq!(functions.iter().filter(|f| f.is_test).count(), 2);
    assert_eq!(functions.iter().filter(|f| f.is_benchmark).count(), 1);
}

#[test]
fn resolve_targets_defaults_and_all() {
    assert_eq!(resolve_test_targets(None), vec!["legion".to_string()]);
    assert_eq!(resolve_test_targets(Some("all")).len(), 4);
}

#[test]
fn test_command_writes_html_report() {
    if !valkyrie_v_root_configured() {
        return;
    }
    let fixture = create_smoke_project_with_manifest(
        "legion-test-report",
        r#"{
    name: "test_app",
    dependencies: {
        "std": false,
        "core": false
    },
    build: [ { target: "ovm" } ]
}
"#,
        r#"micro main() -> i64 {
    return 0
}
"#,
    );
    write_test_suite(&fixture.project_dir);

    let _status = run(&TestArgs {
        project_dir: fixture.project_dir.clone(),
        filter: None,
        target: Some("legion".into()),
        runner: Vec::new(),
        verbose: true,
        standalone: false,
    });

    let html_path = fixture.project_dir.join("dist/legion-test/index.html");
    assert!(html_path.is_file(), "missing HTML report at {}", html_path.display());
    let html = fs::read_to_string(&html_path).unwrap();
    assert!(html.contains("Test Report"));
    assert!(html.contains("add_two") || html.contains("always_pass") || html.contains("编译"));
    assert!(!html.contains("asgard-runtime.js"));
    assert!(!html.contains("LEGION_REPORT_SLOT"));
}

#[test]
fn test_plan_merges_test_files_into_entry_semantic_group() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-group",
        r#"{
    name: "group_app",
    auto_link: { core: false, std: false },
    build: [ { target: "ovm" } ]
}
"#,
        r#"micro main() -> i64 {
    return 0
}
"#,
    );
    write_test_suite(&fixture.project_dir);
    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let target = parse_target_label("ovm").unwrap();
    let (plan, _mode) = workspace
        .build_test_plan(&BuildRequest { project_dir: fixture.project_dir.clone(), target, output_dir: None, enabled_features: Default::default() })
        .expect("test plan generation");

    assert!(plan.project.is_test, "test plan must be marked as a test build");
    let entry_group = plan.project.semantic_source_groups.iter().find(|group| group.is_entry).expect("entry semantic group");
    assert!(
        entry_group.source_files.iter().any(|path| path.ends_with(std::path::Path::new("test").join("suite.v"))),
        "entry group must compile the test sources: {:?}",
        entry_group.source_files
    );
    assert!(
        entry_group.source_files.iter().any(|path| path.ends_with(std::path::Path::new("source").join("main.v"))),
        "entry group must keep the project sources: {:?}",
        entry_group.source_files
    );
    assert_eq!(plan.project.test_entries, vec!["add_two".to_string(), "always_pass".to_string()]);
    assert!(plan.project.source_files.iter().any(|path| path.ends_with(std::path::Path::new("test").join("suite.v"))));
}

#[test]
fn test_plan_rejects_duplicate_test_entry_names() {
    let fixture = create_smoke_project_with_manifest(
        "legion-test-duplicate",
        r#"{
    name: "duplicate_app",
    auto_link: { core: false, std: false },
    build: [ { target: "ovm" } ]
}
"#,
        r#"micro main() -> i64 {
    return 0
}
"#,
    );
    let test_dir = fixture.project_dir.join("test");
    fs::create_dir_all(&test_dir).unwrap();
    fs::write(
        test_dir.join("suite.v"),
        r#"[test]
micro repeated_name() -> i64 {
    return 0
}

[test]
micro repeated_name() -> i64 {
    return 1
}
"#,
    )
    .unwrap();

    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let target = parse_target_label("ovm").unwrap();
    let error = workspace
        .build_test_plan(&BuildRequest { project_dir: fixture.project_dir.clone(), target, output_dir: None, enabled_features: Default::default() })
        .expect_err("duplicate test names must not enter a build plan");
    let message = error.to_string();
    assert!(message.contains("declared more than once"), "unexpected diagnostic: {message}");
    assert!(message.contains("repeated_name"), "diagnostic must name the duplicated test: {message}");
}

#[test]
fn test_filter_skips_non_matching() {
    if !valkyrie_v_root_configured() {
        return;
    }
    let fixture = create_smoke_project_with_manifest(
        "legion-test-filter",
        r#"{
    name: "filter_app",
    dependencies: {
        "std": false,
        "core": false
    },
    build: [ { target: "ovm" } ]
}
"#,
        r#"micro main() -> i64 {
    return 0
}
"#,
    );
    write_test_suite(&fixture.project_dir);

    let status = run(&TestArgs {
        project_dir: fixture.project_dir.clone(),
        filter: Some("no_such_test".into()),
        target: Some("legion".into()),
        runner: Vec::new(),
        verbose: true,
        standalone: false,
    })
    .unwrap();
    assert_eq!(status, ExitCode::SUCCESS);

    let html = fs::read_to_string(fixture.project_dir.join("dist/legion-test/index.html")).unwrap();
    assert!(html.contains("跳过") || html.contains("skip") || html.contains("被过滤器排除") || html.contains("summary-card"));
}

fn write_fixture_file(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// `legion test` 与 `legion build` 共用 workspace 发现事实：`legions.von` 成员
/// 落在 `valkyrie.v` 时，报告模板根不再依赖 `VALKYRIE_V_ROOT`。
#[test]
fn test_entry_reuses_workspace_discovery_without_valkyrie_v_root() {
    let temp = tempfile::Builder::new().prefix("legion-test-workspace-roots").tempdir().unwrap();
    let workspace_root = temp.path().join("workspace");
    let app_dir = workspace_root.join("app");
    let valkyrie_v = temp.path().join("valkyrie.v");

    write_fixture_file(
        &workspace_root.join("legions.von"),
        r#"{
    name: "leetcode",
    members: ["app", "../valkyrie.v/projects/core"]
}
"#,
    );
    write_fixture_file(
        &app_dir.join("legion.von"),
        r#"{
    name: "app",
    auto_link: { core: false, std: false },
    build: [{ target: "ovm" }]
}
"#,
    );
    write_fixture_file(
        &app_dir.join("source").join("main.v"),
        r#"micro main() -> i64 {
    return 0
}
"#,
    );
    write_fixture_file(
        &valkyrie_v.join("legions.von"),
        r#"{
    name: "valkyrie",
    members: ["projects/core", "projects/legion._"]
}
"#,
    );
    write_fixture_file(&valkyrie_v.join("projects").join("core").join("legion.von"), r#"{ name: "core" }"#);
    write_fixture_file(
        &valkyrie_v.join("projects").join("legion._").join("projects").join("legion.report").join("legion.von"),
        r#"{ name: "legion.report" }"#,
    );

    let workspace = WorkspaceResolver::discover_for_project(&app_dir).expect("discover workspace from project dir");
    let expected_root = fs::canonicalize(&valkyrie_v).unwrap();
    let discovered = workspace.source_library_roots();
    assert!(discovered.iter().any(|root| root == &expected_root), "workspace 成员路径必须暴露 valkyrie.v 源码库根: {discovered:?}");

    let roots = report_roots(Some(&workspace), &app_dir).expect("report roots from workspace facts");
    assert!(roots.iter().any(|root| root == &expected_root), "workspace 事实必须优先于环境变量: {roots:?}");
    let report_project = legion_report_project_dir(&roots).expect("report project from workspace roots");
    assert!(
        report_project.ends_with(Path::new("projects").join("legion._").join("projects").join("legion.report")),
        "unexpected report project: {}",
        report_project.display()
    );

    let target = parse_target_label("legion").unwrap();
    let (plan, _mode) = workspace
        .build_test_plan(&BuildRequest { project_dir: app_dir.clone(), target, output_dir: None, enabled_features: Default::default() })
        .expect("test plan generation");
    assert!(plan.project.source_files.iter().any(|path| path.ends_with(Path::new("source").join("main.v"))));
}
