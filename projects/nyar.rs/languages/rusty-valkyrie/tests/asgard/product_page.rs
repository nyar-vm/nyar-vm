//! 真实产品页交付：多区布局、组件级样式、状态流转与真实交互的静态交付产物合同。
//!
//! 夹具是一个运维控制台页面（顶栏 + 英雄区 + 指标卡片 + 标签面板 + 状态切换），
//! 不是文本行堆叠：布局经 `Column`/`Row` 框架组件表达，交互是标签页切换
//! （三个 handler 改写面板正文）与状态流转（两个 handler 在「运行中/维护中」
//! 之间迁移）。本测试经 CLI `--static` 交付路径真实构建，断言：
//! - 页面渲染出全部区块与初始状态；
//! - 首屏关键样式内联进页面；
//! - 每个交互 handler 与响应式状态都落成 wasm 导出符号（产物内接线，非名字表）；
//! - css 按组件 chunk 登记进 manifest。

use std::{fs, process::ExitCode};

use rusty_valkyrie::asgard::cli::{AsgardBuildArgs, AsgardCommands, AsgardDelivery, run};
use tempfile::TempDir;

use crate::support::write_legion_project;

/// 产品页夹具：运维控制台（多区布局 + 标签切换 + 状态流转）。
///
/// 布局与样式全部经样式合同 DSL 表达（root / slot / #id 三类选择器 + 封闭属性集），
/// 页面源码不出现任何 CSS 专有名词。
const PRODUCT_PAGE: &str = r#"<widget index>
<Column slot="page">
    <Row slot="topbar">
        <Text id="brand">Nimbus Panel</Text>
        <Row slot="nav">
            <Text id="nav-item">概览</Text>
            <Text id="nav-item">文档</Text>
        </Row>
    </Row>
    <Column slot="hero">
        <Text id="hero-title">Nimbus 集群控制台</Text>
        <Text id="hero-sub">轻量运维面板 · 静态交付演示</Text>
    </Column>
    <Row slot="cards">
        <Column slot="card">
            <Text id="card-value">12</Text>
            <Text id="card-label">在线节点</Text>
        </Column>
        <Column slot="card">
            <Text id="card-value">34%</Text>
            <Text id="card-label">平均负载</Text>
        </Column>
        <Column slot="card">
            <Text id="card-value">0</Text>
            <Text id="card-label">24h 故障</Text>
        </Column>
    </Row>
    <Column slot="panel">
        <Row slot="tabs">
            <Button id="tab" @click="tab_overview">概览</Button>
            <Button id="tab" @click="tab_alerts">告警</Button>
            <Button id="tab" @click="tab_history">历史</Button>
        </Row>
        <Text id="panel-body">{panel_body}</Text>
        <Row slot="status">
            <Text id="status-label">系统状态</Text>
            <Text id="status-value">{status_text}</Text>
            <Button id="status-action" @click="mark_maintenance">标记维护</Button>
            <Button id="status-action" @click="restore_running">恢复运行</Button>
        </Row>
    </Column>
</Column>
</widget>
<script>
let mut panel_body: utf8 = "12 个节点全部在线，平均负载 34%。"
let mut status_text: utf8 = "运行中"
micro tab_overview() {
    panel_body = "12 个节点全部在线，平均负载 34%。"
}
micro tab_alerts() {
    panel_body = "1 条告警：node-7 内存占用 82%。"
}
micro tab_history() {
    panel_body = "最近 24 小时：0 次故障，2 次发布。"
}
micro mark_maintenance() {
    status_text = "维护中"
}
micro restore_running() {
    status_text = "运行中"
}
</script>
<style>
root { direction: vertical; spacing: 24; inset: 24; background: @surface; width: fill; height: fill }
slot topbar { direction: horizontal; main-align: space-between; cross-align: center }
slot nav { direction: horizontal; spacing: 16 }
slot hero { direction: vertical; spacing: 8; inset: 32 }
slot cards { direction: horizontal; spacing: 16 }
slot card { direction: vertical; spacing: 6; inset: 16; background: @panel; radius: 12; width: fill }
slot panel { direction: vertical; spacing: 14; inset: 20; background: @panel; radius: 12 }
slot tabs { direction: horizontal; spacing: 12 }
slot status { direction: horizontal; spacing: 12; cross-align: center }
#brand { text: @title; color: @brand }
#nav-item { text: @caption; color: @ink-muted }
#hero-title { text: @hero; color: @ink }
#hero-sub { text: @caption; color: @ink-muted }
#card-value { text: @value; color: @brand }
#card-label { text: @caption; color: @ink-muted }
#tab { text: @caption; color: @ink-soft; background: @panel-hover; radius: 8; inset: 6 }
#panel-body { text: @body; color: @ink-soft }
#status-label { text: @caption; color: @ink-muted }
#status-value { text: @caption; color: @ok }
#status-action { text: @caption; color: @ink; background: @brand-strong; radius: 8; inset: 6 }
state hover on #tab { background: @panel }
state pressed on #status-action { background: @brand }
</style>
"#;

/// 合法 Asgard 配置（准入所需；配置求值接入前内容不被消费）。
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

/// 经 CLI `--static` 交付路径构建产品页，返回输出目录。
fn build_product_page(temp: &TempDir) {
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), PRODUCT_PAGE).expect("写入产品页入口失败");
    write_legion_project(temp.path(), "product-page");
    fs::write(temp.path().join("asgard.config.v"), CONFIG).expect("写入 asgard.config.v 失败");
    let args = AsgardBuildArgs {
        project_dir: temp.path().to_path_buf(),
        output_dir: None,
        target: None,
        delivery: AsgardDelivery::Static { entry: "source/index.w".into(), title: Some("Nimbus Panel".into()) },
    };
    let code = run(&AsgardCommands::Build(args)).expect("产品页静态交付构建应成功");
    assert_eq!(code, ExitCode::SUCCESS);
}

fn output_dir(temp: &TempDir) -> std::path::PathBuf {
    temp.path().join(".cache").join("asgard")
}

#[test]
fn product_page_renders_all_sections_with_initial_state() {
    let temp = TempDir::new().expect("创建临时目录失败");
    build_product_page(&temp);
    let html = fs::read_to_string(output_dir(&temp).join("index.html")).expect("读取 index.html 失败");
    assert!(html.contains("<title>Nimbus Panel</title>"), "页面标题应写入:\n{html}");
    for marker in ["Nimbus Panel", "Nimbus 集群控制台", "在线节点", "平均负载", "概览", "告警", "历史", "系统状态"] {
        assert!(html.contains(marker), "页面应渲染区块 `{marker}`:\n{html}");
    }
    assert!(html.contains("12 个节点全部在线，平均负载 34%。"), "面板初始状态应渲染:\n{html}");
    assert!(html.contains(">运行中<"), "状态初始值应渲染:\n{html}");
    assert!(html.contains("data-component=\"index\""), "页面应声明岛挂载点:\n{html}");
}

#[test]
fn product_page_inlines_critical_styles() {
    let temp = TempDir::new().expect("创建临时目录失败");
    build_product_page(&temp);
    let html = fs::read_to_string(output_dir(&temp).join("index.html")).expect("读取 index.html 失败");
    assert!(html.contains("<style>"), "首屏关键样式应内联:\n{html}");
    for rule in [".sc-root { display: flex; flex-direction: column;", ".sc-slot-cards { display: flex; flex-direction: row;", ".sc-id-brand { font-size: 20px; font-weight: 700; line-height: 1.3; color: #38bdf8; }"] {
        assert!(html.contains(rule), "内联样式应覆盖产品页合同降级规则 `{rule}`:\n{html}");
    }
}

#[test]
fn product_page_wasm_exports_every_handler_and_signal() {
    // 每个交互 handler 与响应式状态都必须落成 wasm 导出符号：
    // 交互链路在产物内可验证，浏览器端点击才能经 glue 命中真实状态。
    let temp = TempDir::new().expect("创建临时目录失败");
    build_product_page(&temp);
    let wat = fs::read_to_string(output_dir(&temp).join("product-page.wat")).expect("读取 wat 失败");
    for handler in ["awsl_call_tab_overview", "awsl_call_tab_alerts", "awsl_call_tab_history", "awsl_call_mark_maintenance", "awsl_call_restore_running"] {
        assert!(wat.contains(handler), "wasm 必须导出交互 handler `{handler}`");
    }
    for signal in ["awsl_sig_index_panel_body", "awsl_sig_index_status_text"] {
        assert!(wat.contains(signal), "wasm 必须导出响应式状态信号 `{signal}`");
    }
}

#[test]
fn product_page_manifest_registers_css_chunk() {
    let temp = TempDir::new().expect("创建临时目录失败");
    build_product_page(&temp);
    let output = output_dir(&temp);
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("manifest.json")).expect("读取 manifest 失败")).expect("manifest 必须是合法 JSON");
    let css = manifest.get("css").and_then(serde_json::Value::as_array).expect("manifest 应声明 css chunk");
    assert_eq!(css.len(), 1, "产品页应恰有一个组件级 css chunk");
    assert_eq!(css[0].as_str(), Some("index.css"), "css chunk 必须按组件名登记");
    let integrity = manifest.get("integrity").and_then(serde_json::Value::as_object).expect("manifest 必须声明摘要表");
    assert!(integrity.contains_key("index.css"), "摘要表必须覆盖 css chunk");
    assert!(output.join("index.css").is_file(), "css chunk 必须落盘");
}

/// 持久产物构建：仅当环境变量指向项目目录时执行，供浏览器截图与交互验证。
///
/// 默认无副作用；执行后产物落在 `<项目目录>/.cache/asgard`（缺省输出目录）。
#[test]
fn product_page_persistent_build_for_browser_verification() {
    let Some(root) = std::env::var_os("ASGARD_PRODUCT_PAGE_PROJECT") else {
        return;
    };
    let project = std::path::PathBuf::from(root);
    fs::create_dir_all(project.join("source")).expect("创建 source/ 目录失败");
    fs::write(project.join("source/index.w"), PRODUCT_PAGE).expect("写入产品页入口失败");
    write_legion_project(&project, "product-page");
    fs::write(project.join("asgard.config.v"), CONFIG).expect("写入 asgard.config.v 失败");
    let args = AsgardBuildArgs {
        project_dir: project.clone(),
        output_dir: None,
        target: None,
        delivery: AsgardDelivery::Static { entry: "source/index.w".into(), title: Some("Nimbus Panel".into()) },
    };
    let code = run(&AsgardCommands::Build(args)).expect("产品页持久构建应成功");
    assert_eq!(code, ExitCode::SUCCESS);
    let output = project.join(".cache").join("asgard");
    assert!(output.join("index.html").is_file(), "持久产物必须落在缺省输出目录");
    println!("product page: {}", output.join("index.html").display());
}
