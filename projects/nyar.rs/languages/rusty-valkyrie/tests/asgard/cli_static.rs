//! CLI 路由测试：`asgard build` 的静态交付走静态站点编排，失败闭合且不回落到应用管线。

use std::{fs, path::PathBuf, process::ExitCode};

use rusty_valkyrie::asgard::cli::{AsgardBuildArgs, AsgardCommands, AsgardDelivery, run};
use tempfile::TempDir;

use crate::support::{legion_manifest, write_legion_project};

/// 带样式与初始状态绑定的根组件入口 fixture。
const FIXTURE: &str = r#"<widget index>
<Column class="hero">
    <Text class="hero-title">{greeting}</Text>
</Column>
</widget>
<script>
let greeting: utf8 = "你好，静态世界"
</script>
<style>
root { color: @brand }
</style>
"#;

/// 合法 Asgard 配置入口。
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

/// 写入项目准入所需的工程文件（工作区清单、项目清单与配置）。
fn write_project_files(project: &std::path::Path) {
    write_legion_project(project, "cli-static");
    fs::write(project.join("asgard.config.v"), CONFIG).expect("写入 asgard.config.v 失败");
}

fn build_args(temp: &TempDir, entry: &str, title: Option<String>) -> AsgardBuildArgs {
    AsgardBuildArgs {
        project_dir: temp.path().to_path_buf(),
        output_dir: None,
        target: None,
        delivery: AsgardDelivery::Static { entry: PathBuf::from(entry), title },
    }
}

#[test]
fn build_routes_static_delivery_to_styled_page() {
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    write_project_files(temp.path());

    let code = run(&AsgardCommands::Build(build_args(&temp, "source/index.w", Some("CLI 静态交付".into())))).expect("CLI 静态交付构建应成功");
    assert_eq!(code, ExitCode::SUCCESS);

    let html = fs::read_to_string(temp.path().join(".cache").join("asgard").join("index.html")).expect("读取 index.html 失败");
    assert!(html.contains("<!DOCTYPE html>"), "应为完整 HTML 页面");
    assert!(html.contains(".sc-root { color: #38bdf8; }"), "应包含合同降级的组件样式:\n{html}");
    assert!(html.contains("你好，静态世界"), "应包含初始状态插值:\n{html}");
    assert!(html.contains("<title>CLI 静态交付</title>"), "应使用显式标题:\n{html}");
    assert!(
        html.contains("<div class=\"hero sc-root\"><span class=\"hero-title\">你好，静态世界</span></div>"),
        "框架组件应降低为 DOM adapter 形态（合同句柄并入根元素 class）:\n{html}"
    );
}

#[test]
fn static_delivery_fails_closed_on_bad_entry() {
    // 非法入口后缀必须失败返回，而不是回落到应用构建管线。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.v"), FIXTURE).expect("写入入口文件失败");
    write_project_files(temp.path());

    let error = run(&AsgardCommands::Build(build_args(&temp, "source/index.v", None))).expect_err("非法入口后缀应失败");
    assert!(error.to_string().contains(".w"), "诊断应说明合法后缀: {error}");
    assert!(!temp.path().join(".cache").join("asgard").join("index.html").exists(), "失败路径不应产出页面");
}

/// 越出 SSG 准入子集的表达式必须结构化失败：不崩溃，也不写出残缺页面。
#[test]
fn static_delivery_fails_closed_on_out_of_subset_expression() {
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    let fixture = r#"<widget index>
<Column class="hero">
    <Text class="hero-title">{active}</Text>
    <Text class="deep">{!active}</Text>
</Column>
</widget>
<script>
let active: bool = true
</script>
"#;
    fs::write(temp.path().join("source/index.w"), fixture).expect("写入越界表达式入口失败");
    write_project_files(temp.path());

    let error = run(&AsgardCommands::Build(build_args(&temp, "source/index.w", None))).expect_err("越界表达式应结构化失败");
    let message = error.to_string();
    assert!(message.contains("SSG 准入子集"), "诊断应说明越出准入子集: {message}");
    assert!(message.contains("表达式形态越出"), "诊断应指出表达式形态越界: {message}");
    assert!(!temp.path().join(".cache").join("asgard").join("index.html").exists(), "失败路径不得产出残缺页面");
}

#[test]
fn dev_rebuild_swaps_the_whole_generation() {
    // dev 成功重建后整代替换：`dist` 内是新一代产物，staging 不残留。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    write_project_files(temp.path());

    let code = run(&AsgardCommands::Dev(build_args(&temp, "source/index.w", Some("dev 重建".into())))).expect("dev 静态重建应成功");
    assert_eq!(code, ExitCode::SUCCESS);
    assert!(temp.path().join(".cache/asgard/index.html").is_file(), "新一代产物必须落到缺省输出目录");
    assert!(temp.path().join(".cache/asgard/manifest.json").is_file(), "整代产物应包含 manifest");
    assert!(!temp.path().join(".cache/asgard.staging").exists(), "staging 目录应被替换掉而不是残留");
    assert!(!temp.path().join(".cache/asgard.previous").exists(), "上一代应在成功后清理");
}

#[test]
fn dev_failure_keeps_the_previous_generation() {
    // 构建失败时上一代产物原地保留，不被半成品覆盖。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::create_dir_all(temp.path().join(".cache").join("asgard")).expect("创建上一代产物目录失败");
    fs::write(temp.path().join(".cache/asgard/index.html"), "<!DOCTYPE html><title>previous generation</title>").expect("写入上一代页面失败");
    fs::write(temp.path().join("source/index.w"), "<widget mismatch></widget>").expect("写入名不匹配的入口失败");
    write_project_files(temp.path());

    let _error = run(&AsgardCommands::Dev(build_args(&temp, "source/index.w", None))).expect_err("入口与根组件名不匹配应失败");
    let previous = fs::read_to_string(temp.path().join(".cache/asgard/index.html")).expect("上一代页面应仍然可读");
    assert!(previous.contains("previous generation"), "失败重建不得覆盖上一代页面: {previous}");
    assert!(!temp.path().join(".cache/asgard.staging").exists(), "失败重建不得把 staging 当成产物");
}

#[test]
fn app_delivery_fails_closed_with_structured_diagnostic() {
    // `App` 交付按设计失败闭合：配置求值（C0/C2）未闭合时不得静默回落到静态路径，
    // 也不得产出任何页面充当成功。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    write_project_files(temp.path());
    let mut args = build_args(&temp, "source/index.w", None);
    args.delivery = AsgardDelivery::App;

    let error = run(&AsgardCommands::Build(args)).expect_err("App 交付应结构化失败");
    let message = error.to_string();
    assert!(message.contains("asgard build (app)"), "诊断应标明 App 交付: {message}");
    assert!(message.contains("配置求值") && message.contains("C0/C2"), "诊断应指出配置求值缺口: {message}");
    assert!(message.contains("--static"), "诊断应指明当前可用入口: {message}");
    assert!(!temp.path().join(".cache").join("asgard").join("index.html").exists(), "App 失败路径不得产出静态页面");
}

#[test]
fn app_delivery_admission_runs_before_pipeline_diagnostic() {
    // 准入是命令面共同前置：缺 `legion.von` 时先报准入，而不是先把 App 缺口当结论。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    fs::write(temp.path().join("asgard.config.v"), CONFIG).expect("写入 asgard.config.v 失败");
    let mut args = build_args(&temp, "source/index.w", None);
    args.delivery = AsgardDelivery::App;

    let error = run(&AsgardCommands::Build(args)).expect_err("缺少 Legion 项目应失败");
    let message = error.to_string();
    assert!(message.contains("legion.von"), "诊断应指出缺失的 Legion 项目清单: {message}");
    assert!(!message.contains("配置求值"), "准入失败应先于 App 管线诊断: {message}");
}

#[test]
fn dev_app_delivery_fails_closed_without_staging() {
    // `dev` 只接通静态交付；`App` 必须结构化拒绝，且不得创建 staging 或动到已有产物。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    write_project_files(temp.path());
    let mut args = build_args(&temp, "source/index.w", None);
    args.delivery = AsgardDelivery::App;

    let error = run(&AsgardCommands::Dev(args)).expect_err("App dev 应结构化失败");
    let message = error.to_string();
    assert!(message.contains("asgard dev"), "诊断应标明 dev 命令: {message}");
    assert!(message.contains("配置求值") && message.contains("C0/C2"), "诊断应指出配置求值缺口: {message}");
    assert!(message.contains("--static"), "诊断应指明静态交付入口: {message}");
    assert!(!temp.path().join(".cache/asgard.staging").exists(), "拒绝路径不得创建 staging 目录");
    assert!(!temp.path().join(".cache").join("asgard").exists(), "拒绝路径不得产出缺省输出目录");
}

#[test]
fn build_requires_legion_project() {
    // 缺少 `legion.von` 的项目不是合法 Legion 项目，命令必须在读取入口前失败。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    fs::write(temp.path().join("asgard.config.v"), CONFIG).expect("写入 asgard.config.v 失败");

    let error = run(&AsgardCommands::Build(build_args(&temp, "source/index.w", None))).expect_err("缺少 legion.von 应失败");
    let message = error.to_string();
    assert!(message.contains("legion.von"), "诊断应指出缺失的 Legion 项目清单: {message}");
    assert!(!temp.path().join(".cache").join("asgard").join("index.html").exists(), "失败路径不应产出页面");
}

#[test]
fn build_requires_asgard_config() {
    // 缺少 `asgard.config.v` 时任何 Asgard 命令都不能运行（`asgard setup` 除外）。
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    fs::write(temp.path().join("legion.von"), legion_manifest("cli-static")).expect("写入 legion.von 失败");

    let error = run(&AsgardCommands::Build(build_args(&temp, "source/index.w", None))).expect_err("缺少 asgard.config.v 应失败");
    let message = error.to_string();
    assert!(message.contains("asgard.config.v"), "诊断应指出缺失的 Asgard 配置: {message}");
    assert!(!temp.path().join(".cache").join("asgard").join("index.html").exists(), "失败路径不应产出页面");
}
