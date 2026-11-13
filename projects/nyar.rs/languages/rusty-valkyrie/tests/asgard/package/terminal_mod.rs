//! Migrated from `src/package/terminal/mod.rs` (`tests`).

use rusty_valkyrie::asgard::{
    codegen::{ScalarBootstrap, build_awsl_terminal_source},
    package::{PackageArtifact, terminal::*},
};

use oak_awsl::AwslParser;
use rusty_valkyrie::asgard::awsl::{LoweredComponent, LoweringOptions, lower_component};

fn demo_component() -> LoweredComponent {
    let source = r#"<widget Demo><text>hi</text></widget>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower")
}

fn demo_app_v() -> String {
    let component = demo_component();
    let out = package_terminal_project(build_awsl_terminal_source(&[component], ScalarBootstrap::ClosedUnit).expect("generate terminal source").source, "app", &[]);
    let (_, artifact) = out.files.iter().find(|(n, _)| n == "app.v").expect("app.v");
    match artifact {
        PackageArtifact::Text(text) => text.clone(),
        _ => panic!("app.v must be text"),
    }
}

#[test]
fn terminal_package_has_all_artifacts() {
    let component = demo_component();
    let out = package_terminal_project(build_awsl_terminal_source(&[component], ScalarBootstrap::ClosedUnit).expect("generate terminal source").source, "app", &[]);
    assert!(out.files.iter().any(|(n, _)| n == "manifest.toml"), "missing manifest.toml");
    assert!(out.files.iter().any(|(n, _)| n == "app.v"), "missing app.v");
    let exe_count = out.files.iter().filter(|(n, _)| n.starts_with("app")).count();
    assert!(exe_count >= 1, "missing executable");
}

#[test]
fn app_v_contains_session_builder() {
    let text = demo_app_v();
    assert!(
        text.contains("micro awsl_build_demo(session: mut TuiSession) -> ArrayList<Widget>"),
        "app.v 必须包含带 session 的控件树构造函数: {text}"
    );
    assert!(text.contains("awsl_text"), "app.v 必须包含 awsl_text 调用");
    assert!(text.contains("asgard_terminal_run(awsl_build_demo)"), "app.v 主入口必须以函数值调用产品入口: {text}");
}

#[test]
fn app_v_uses_frozen_terminal_namespaces() {
    let text = demo_app_v();
    assert!(text.contains("namespace asgard.generated;"), "生成段必须落在 asgard.generated 命名空间: {text}");
    assert!(text.contains("using std.terminal;"), "生成段必须显式导入 std.terminal: {text}");
    assert!(text.contains("using asgard.terminal;"), "生成段必须显式导入 asgard.terminal: {text}");
    // host_contract 归 std.terminal 唯一所有：生成单元不复制运行时声明。
    assert!(!text.contains("[host_contract]"), "app.v 不得复制 host_contract 声明: {text}");
    assert!(!text.contains("structure TuiRuntime"), "旧 TuiRuntime 全局运行时未清退: {text}");
}

#[test]
fn manifest_declares_terminal_platform() {
    let component = demo_component();
    let out = package_terminal_project(build_awsl_terminal_source(&[component], ScalarBootstrap::ClosedUnit).expect("generate terminal source").source, "app", &[]);
    let (_, artifact) = out.files.iter().find(|(n, _)| n == "manifest.toml").expect("manifest.toml");
    let text = match artifact {
        PackageArtifact::Text(text) => text,
        _ => panic!("manifest must be text"),
    };
    assert!(text.contains("platform: terminal"), "manifest must declare platform: terminal");
}

#[test]
fn no_desktop_or_mobile_artifacts() {
    let component = demo_component();
    let out = package_terminal_project(build_awsl_terminal_source(&[component], ScalarBootstrap::ClosedUnit).expect("generate terminal source").source, "app", &[]);
    for (name, _) in &out.files {
        let lower = name.to_ascii_lowercase();
        assert!(!lower.contains("index.html"), "unexpected index.html: {name}");
        assert!(!lower.contains("classes.dex"), "unexpected classes.dex: {name}");
        assert!(!lower.contains("asgardhost"), "unexpected AsgardHost: {name}");
        assert!(!lower.contains("asgardterminalruntime.c"), "unexpected C runtime: {name}");
    }
}

#[test]
fn manifest_declares_app_v_source() {
    let component = demo_component();
    let out = package_terminal_project(build_awsl_terminal_source(&[component], ScalarBootstrap::ClosedUnit).expect("generate terminal source").source, "app", &[]);
    let (_, artifact) = out.files.iter().find(|(n, _)| n == "manifest.toml").expect("manifest.toml");
    let text = match artifact {
        PackageArtifact::Text(text) => text,
        _ => panic!("manifest must be text"),
    };
    assert!(text.contains("source: app.v"), "manifest must declare source: app.v");
}
