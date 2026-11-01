//! 目标策略与 chunk 分割规划：浏览器保留分割计划，桌面 WebView 直接跳过。
//!
//! 验收要求来自 [03 §7]：所有宿主都必须按语义可达性减枝；桌面 WebView 在 target
//! 策略中跳过 chunk partitioning，不计算浏览器 chunk 拓扑，也不生成动态 loader/
//! manifest。作者配置的 `artifact.chunks` 仍应被严格模型校验，但桌面交付不消费它。

use std::{fs, path::Path};

use rusty_valkyrie::asgard::{
    chunk_planning::{
        ChunkPartitioning, DeliveryHost, PruningStrategy, ensure_browser_chunk_delivery_supported, plan_asgard_build, resolve_chunk_graph,
    },
    config_model::{ConfigOrigin, ConfigValue, ResolvedAsgardConfig, resolve_static_config},
};
use tempfile::TempDir;

fn obj(fields: Vec<(&str, ConfigValue)>) -> ConfigValue {
    ConfigValue::Object(fields.into_iter().map(|(name, value)| (name.to_owned(), value)).collect())
}

fn text(value: &str) -> ConfigValue {
    ConfigValue::Text(value.to_owned())
}

fn boundary(component: &str, preload: bool, prefetch: bool) -> ConfigValue {
    obj(vec![("component", text(component)), ("preload", ConfigValue::Bool(preload)), ("prefetch", ConfigValue::Bool(prefetch))])
}

fn budgets(entry_bytes: i64, chunk_bytes: i64) -> ConfigValue {
    obj(vec![("entry_bytes", ConfigValue::Int(entry_bytes)), ("chunk_bytes", ConfigValue::Int(chunk_bytes))])
}

/// 含两条边界与预算的完整配置。
fn configured_chunks() -> ConfigValue {
    obj(vec![
        ("boundaries", ConfigValue::List(vec![boundary("Admin", true, false), boundary("Docs", false, true)])),
        ("budgets", budgets(1024, 512)),
    ])
}

fn resolve_config(temp: &Path, chunks: ConfigValue) -> ResolvedAsgardConfig {
    let entry = temp.join("source").join("App.w");
    fs::create_dir_all(entry.parent().expect("入口应有父目录")).expect("创建入口目录失败");
    fs::write(&entry, "<widget></widget>\n").expect("写入入口文件失败");
    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w")), ("chunks", chunks)]))]);
    let origin = ConfigOrigin { config_path: temp.join("asgard.config.v"), project_root: temp.to_path_buf() };
    resolve_static_config(value, origin).expect("chunks 配置必须被严格模型接受")
}

#[test]
fn browser_delivery_keeps_configured_chunk_partitioning() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let config = resolve_config(temp.path(), configured_chunks());
    let plan = config.build_plan(DeliveryHost::BrowserHttp);
    assert_eq!(plan.host, DeliveryHost::BrowserHttp);
    assert_eq!(plan.pruning, PruningStrategy::SemanticReachability, "所有宿主都必须按语义减枝");
    let ChunkPartitioning::Partitioned(chunks) = &plan.chunk_partitioning
    else {
        panic!("浏览器交付必须保留分割计划");
    };
    assert_eq!(chunks.boundaries.len(), 2, "配置边界必须按声明顺序保留");
    assert_eq!(chunks.boundaries[0].component, "Admin");
    assert!(chunks.boundaries[0].preload && !chunks.boundaries[0].prefetch);
    assert_eq!(chunks.budgets.entry_bytes, Some(1024));
    assert_eq!(chunks.budgets.chunk_bytes, Some(512));
    assert!(plan.chunk_partitioning.requires_chunk_loader(), "浏览器分割交付需要 chunk loader");
}

#[test]
fn webview_skips_chunk_partitioning_even_when_configured() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let config = resolve_config(temp.path(), configured_chunks());
    let plan = config.build_plan(DeliveryHost::DesktopWebView);
    assert_eq!(plan.host, DeliveryHost::DesktopWebView);
    assert_eq!(plan.pruning, PruningStrategy::SemanticReachability, "桌面同样必须按语义减枝");
    assert_eq!(plan.chunk_partitioning, ChunkPartitioning::Skipped, "桌面 target 必须跳过分割规划");
    assert!(!plan.chunk_partitioning.is_partitioned());
    assert!(plan.chunk_partitioning.boundaries().is_empty(), "桌面不计算 chunk 拓扑");
    assert!(plan.chunk_partitioning.budgets().is_none(), "桌面不承担 chunk 预算门禁");
    assert!(!plan.chunk_partitioning.requires_chunk_loader(), "桌面不得生成动态 chunk loader/manifest");
}

#[test]
fn webview_without_chunks_config_still_skips_partitioning() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let config = resolve_config(temp.path(), obj(vec![]));
    let plan = plan_asgard_build(DeliveryHost::DesktopWebView, &config.chunks);
    assert_eq!(plan.chunk_partitioning, ChunkPartitioning::Skipped);
    assert!(plan.chunk_partitioning.plan().is_none());
}

#[test]
fn chunk_graph_keeps_declaration_order_with_entry_edges() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let config = resolve_config(temp.path(), configured_chunks());
    let graph = resolve_chunk_graph(&config.chunks, "index").expect("合法边界应解析为依赖图");
    assert_eq!(graph.entry, "index");
    assert_eq!(graph.nodes.len(), 2, "每条边界一个按需节点");
    assert_eq!(graph.nodes[0].component, "Admin");
    assert_eq!(graph.nodes[1].component, "Docs", "节点顺序必须保持声明顺序");
    assert_eq!(graph.nodes[0].depends_on, vec!["index".to_string()], "按需 chunk 至少依赖入口闭包");
    assert!(graph.nodes[0].preload && !graph.nodes[0].prefetch);
    assert!(!graph.nodes[1].preload && graph.nodes[1].prefetch);
}

#[test]
fn chunk_graph_serializes_the_loader_contract_shape() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let config = resolve_config(temp.path(), configured_chunks());
    let graph = resolve_chunk_graph(&config.chunks, "index").expect("合法边界应解析为依赖图");
    // 冻结 loader 消费的字段形状：entry + nodes[component/depends_on/preload/prefetch]。
    let value = serde_json::to_value(&graph).expect("依赖图必须可序列化");
    assert_eq!(
        value,
        serde_json::json!({
            "entry": "index",
            "nodes": [
                { "component": "Admin", "depends_on": ["index"], "preload": true, "prefetch": false },
                { "component": "Docs", "depends_on": ["index"], "preload": false, "prefetch": true }
            ]
        }),
        "依赖图 JSON 形状即 loader 数据合同，字段名不得漂移"
    );
}

#[test]
fn chunk_graph_rejects_duplicate_and_entry_boundaries() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let duplicated = ConfigValue::List(vec![boundary("Admin", false, false), boundary("Admin", true, true)]);
    let config = resolve_config(temp.path(), obj(vec![("boundaries", duplicated)]));
    let error = resolve_chunk_graph(&config.chunks, "index").expect_err("重复边界组件必须拒绝");
    assert!(error.to_string().contains("重复声明"), "诊断应说明边界重复: {error}");

    let entry_as_boundary = ConfigValue::List(vec![boundary("index", false, false)]);
    let config = resolve_config(temp.path(), obj(vec![("boundaries", entry_as_boundary)]));
    let error = resolve_chunk_graph(&config.chunks, "index").expect_err("入口组件不得再声明为按需边界");
    assert!(error.to_string().contains("入口"), "诊断应说明与入口重复: {error}");
}

#[test]
fn browser_delivery_with_declared_boundaries_fails_closed_until_chunk_artifacts_exist() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let config = resolve_config(temp.path(), configured_chunks());
    let plan = config.build_plan(DeliveryHost::BrowserHttp);
    let error = ensure_browser_chunk_delivery_supported(&plan).expect_err("带边界的浏览器计划必须失败闭合");
    let message = error.to_string();
    assert!(message.contains("chunk"), "诊断应指明 chunk 缺口: {message}");
    assert!(message.contains("尚未实施"), "诊断应说明缺口状态而不是冒充成功: {message}");

    // 未声明边界的浏览器计划与桌面未分割交付放行。
    let empty = resolve_config(temp.path(), obj(vec![]));
    ensure_browser_chunk_delivery_supported(&empty.build_plan(DeliveryHost::BrowserHttp)).expect("无边界的浏览器计划应放行");
    ensure_browser_chunk_delivery_supported(&empty.build_plan(DeliveryHost::DesktopWebView)).expect("桌面未分割交付应放行");
}
