//! Migrated from `src/cmds/report/render.rs` (`tests`).

use super::*;

use tempfile::tempdir;

use crate::legion::cmds::report::models::{BenchResultRow, CoverageFeatureEntry};

/// 报告渲染依赖 `valkyrie.v` 源码库中的 asgard.plotter 资产；CI 没有该兄弟仓，
/// 未配置 `VALKYRIE_V_ROOT` 时跳过而不是把外部仓依赖伪装成失败。
fn valkyrie_v_root_configured() -> bool {
    match std::env::var_os("VALKYRIE_V_ROOT") {
        Some(root) if !root.is_empty() => true,
        _ => {
            eprintln!("skip: VALKYRIE_V_ROOT 未配置，报告渲染需要 valkyrie.v 资产");
            false
        }
    }
}

#[test]
fn render_test_report_uses_asgard_islands_not_handwritten_js() {
    if !valkyrie_v_root_configured() {
        return;
    }
    let dir = tempdir().unwrap();
    let results = vec![
        TestResultEntry { name: "add".into(), status: "pass".into(), error: None, target: "legion".into() },
        TestResultEntry { name: "sub".into(), status: "fail".into(), error: Some("boom".into()), target: "legion".into() },
    ];
    render_test_report(dir.path(), "demo", &results).unwrap();
    let html = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
    assert!(html.contains("demo - Test Report"));
    assert!(html.contains("data-island=\"hydrated\""));
    assert!(html.contains("data-component=\"chart-status\""));
    assert!(html.contains(r#"src="boot.js""#));
    assert!(!html.contains("__voa.start"));
    assert!(!html.contains("globalThis.__voa"));
    assert!(dir.path().join("c/chart-status.js").exists());
    assert!(dir.path().join("boot.js").exists());
    assert!(dir.path().join("manifest.json").exists());
    let boot = std::fs::read_to_string(dir.path().join("boot.js")).unwrap();
    assert!(boot.contains("__voa"));
    assert!(boot.contains("start('manifest.json')"));
    let glue = std::fs::read_to_string(dir.path().join("c/chart-status.js")).unwrap();
    assert!(glue.contains("callExport"));
    assert!(!glue.contains("innerHTML"));
    assert!(!boot.contains("plotter-interactive"));
    assert!(!html.contains("plotter-interactive"));
    assert!(!html.contains("asgard-runtime.js"));
    assert!(!html.contains(CHART_STATUS));
    // UI comes from asgard.plotter (styles packaged), not Rust-synthesized charts.
    let css_path = dir.path().join("legion-test.css");
    if css_path.exists() {
        let css = std::fs::read_to_string(css_path).unwrap();
        assert!(css.contains("asgard-icol") || css.contains("asgard-icol-bar"));
    }
}

#[test]
fn render_coverage_report_emits_islands() {
    if !valkyrie_v_root_configured() {
        return;
    }
    let dir = tempdir().unwrap();
    let report = CoverageReport {
        covered: 1,
        total: 2,
        percentage: 50.0,
        features: vec![
            CoverageFeatureEntry { feature: "micro".into(), display: "micro 函数".into(), covered: true, projects: vec!["app".into()] },
            CoverageFeatureEntry { feature: "match".into(), display: "模式匹配".into(), covered: false, projects: vec![] },
        ],
    };
    render_coverage_report(dir.path(), &report).unwrap();
    let html = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
    assert!(html.contains("Coverage Report"));
    assert!(html.contains("data-component=\"chart-coverage-balance\""));
    assert!(html.contains("data-component=\"chart-coverage-features\""));
    assert!(html.contains("boot.js"));
    assert!(!html.contains("plotter-interactive"));
}

#[test]
fn render_bench_report_emits_islands() {
    if !valkyrie_v_root_configured() {
        return;
    }
    let dir = tempdir().unwrap();
    let report = BenchReport {
        runs: 3,
        rows: vec![BenchResultRow { project: "app".into(), test: "fib".into(), target: "legion".into(), compile_ms: 12.5, runtime_ms: 1.2 }],
    };
    render_bench_report(dir.path(), &report).unwrap();
    let html = std::fs::read_to_string(dir.path().join("index.html")).unwrap();
    assert!(html.contains("Benchmark Report"));
    assert!(html.contains("data-component=\"chart-bench-runtime\""));
    assert!(html.contains("data-component=\"chart-bench-compile\""));
    assert!(dir.path().join("boot.js").exists());
    assert!(!html.contains("plotter-interactive"));
}

