//! `asgard build` 默认 App 交付与应用交付 `asgard dev` 的失败闭合边界。
//!
//! App 交付在项目/配置准入之后明确失败，而不是回落到静态交付或伪造应用构建。
//! 静态 dev 的 staging 整代替换由 `cli_static` 覆盖；这里锁定 App/dev 应用路径，
//! 断言语义：项目与配置存在并不足以放行，缺口是配置值求值（`ConfigValue` 无生产者）。

use std::fs;

use rusty_valkyrie::asgard::cli::{AsgardBuildArgs, AsgardCommands, AsgardDelivery, run};
use tempfile::TempDir;

/// 合法 Legion 项目清单元数据。
const MANIFEST: &str = r#"{
    name: "cli-app-dev",
    version: "0.1.0",
    build: [
        { target: "wasm32-unknown-browser-wasm" }
    ]
}
"#;

/// 合法 Asgard 配置入口；App 与 dev 尚未读取其内容。
const CONFIG: &str = r#"using asgard;

define_config(AsgardConfig {
    artifact: ArtifactConfig {
        target: Some(ArtifactTarget.Static)
        entry: "source/index.w"
        output: Some(".cache/asgard")
        base: Some("/")
        chunks: None
    }
    assets: None
    build: None
    serve: None
    dev: None
})
"#;

/// 最小根组件入口，证明失败不是入口缺失导致。
const FIXTURE: &str = "<widget><Text>app-dev</Text></widget>\n";

/// 写入准入所需的两份工程文件、入口文件，返回参数。
fn project_with(delivery: AsgardDelivery) -> (TempDir, AsgardBuildArgs) {
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    fs::write(temp.path().join("legion.von"), MANIFEST).expect("写入 legion.von 失败");
    fs::write(temp.path().join("asgard.config.v"), CONFIG).expect("写入 asgard.config.v 失败");
    let args = AsgardBuildArgs { project_dir: temp.path().to_path_buf(), output_dir: None, target: None, delivery };
    (temp, args)
}

/// 失败闭合时不得写出任何页面或产物目录。
fn assert_no_artifacts(temp: &TempDir) {
    assert!(!temp.path().join(".cache").join("asgard").exists(), "失败闭合不得创建缺省输出目录");
    assert!(!temp.path().join(".cache").join("asgard").join("index.html").exists(), "失败闭合不得写出页面");
}

#[test]
fn app_build_fails_closed_until_config_evaluation_is_connected() {
    // 默认交付就是 App，必须保持失败闭合，不能静默降级为静态站点。
    assert!(matches!(AsgardDelivery::default(), AsgardDelivery::App));
    let (temp, args) = project_with(AsgardDelivery::App);
    let error = run(&AsgardCommands::Build(args)).expect_err("App 构建应在配置求值缺口处失败");
    let message = error.to_string();
    assert!(message.contains("配置求值"), "诊断应指出配置求值缺口: {message}");
    assert!(message.contains("C0/C2"), "诊断应保留求值阶段坐标: {message}");
    assert!(message.contains("build --static"), "诊断应指向唯一可用的静态交付入口: {message}");
    assert_no_artifacts(&temp);
}

#[test]
fn dev_app_delivery_fails_closed_until_config_evaluation_is_connected() {
    let (temp, args) = project_with(AsgardDelivery::App);
    let error = run(&AsgardCommands::Dev(args)).expect_err("应用交付的 dev 应在配置求值缺口处失败");
    let message = error.to_string();
    assert!(message.contains("配置求值"), "诊断应指出配置求值缺口: {message}");
    assert!(message.contains("C0/C2"), "诊断应保留求值阶段坐标: {message}");
    assert!(message.contains("asgard dev"), "诊断应指明 dev 命令: {message}");
    assert_no_artifacts(&temp);
}

#[test]
fn app_build_requires_project_admission_before_evaluation_error() {
    // 准入先于求值缺口诊断：缺 `legion.von` 时错误必须是准入失败，而不是 App/dev 缺口。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::write(temp.path().join("asgard.config.v"), CONFIG).expect("写入 asgard.config.v 失败");
    let args = AsgardBuildArgs { project_dir: temp.path().to_path_buf(), output_dir: None, target: None, delivery: AsgardDelivery::App };
    let error = run(&AsgardCommands::Build(args)).expect_err("缺少 legion.von 应失败");
    let message = error.to_string();
    assert!(message.contains("legion.von"), "应先报告 Legion 项目准入失败: {message}");
    assert!(!message.contains("C0/C2"), "准入失败时不得冒充求值缺口: {message}");
}
