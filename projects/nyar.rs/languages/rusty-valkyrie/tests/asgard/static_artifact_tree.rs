//! `build --static` 产物树合同：文件布局、页面↔产物引用一致性、配置字段消费。
//!
//! 静态交付线现在编排完整浏览器产物：AWSL 初始渲染页面 + WASM chunk + 浏览器 glue +
//! 组件胶水 + `boot.js` + `manifest.json` + CSS。本文件固化的真实形态：
//! - 产物树 = 页面 + 胶囊产物（顺序无关，集合相等）；
//! - 页面内 `src`/`href` 引用必须解析到产物树内真实文件；
//! - `manifest.json` 引用的 wasm / glue / css / 组件 glue 必须全部存在；
//! - 严格模型校验后的 `artifact.base` 已作为构建输入被消费（写入 `<base href>`）。

use std::{fs, path::PathBuf};

use rusty_valkyrie::asgard::{
    config_model::{ConfigOrigin, ConfigValue, resolve_static_config},
    static_site::{StaticSiteOptions, build_static_site},
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::support::write_legion_project;

/// 带样式与初始状态绑定的根组件入口 fixture。
const FIXTURE: &str = r#"<widget index>
<Column class="hero">
    <Text class="hero-title">{greeting}</Text>
</Column>
</widget>
<script>
let greeting: utf8 = "静态产物树"
</script>
<style>
root { color: @ink-muted }
</style>
"#;

/// 在项目根写入入口并构建，返回输出目录内的文件清单（相对输出根，`/` 分隔）。
fn build_and_list(project: &std::path::Path, output: &std::path::Path, base: &str) -> Vec<String> {
    fs::create_dir_all(project.join("source")).expect("创建 source/ 目录失败");
    write_legion_project(project, "static-artifact-fixture");
    fs::write(project.join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    let report = build_static_site(&StaticSiteOptions {
        project_dir: project.to_path_buf(),
        entry: PathBuf::from("source/index.w"),
        output_dir: Some(output.to_path_buf()),
        title: Some("产物树测试".into()),
        base: base.to_string(),
        assets_dir: None,
    })
    .expect("静态站点构建应成功");
    assert_eq!(report.page_path, output.join("index.html"), "报告页面路径应落在输出目录");
    let mut files = Vec::new();
    collect_files(output, output, &mut files);
    files.sort();
    files
}

fn collect_files(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("读取输出目录失败") {
        let path = entry.expect("目录项").path();
        if path.is_dir() {
            collect_files(root, &path, out);
        }
        else {
            out.push(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn static_build_output_tree_contains_paired_browser_artifacts() {
    // 真实形态：页面 + wasm chunk + 浏览器 glue + 组件胶水 + boot + manifest + CSS。
    // 断言集合相等而非存在性，避免"目录里有 wasm"冒充交付。
    let temp = TempDir::new().expect("创建临时目录失败");
    let files = build_and_list(temp.path(), &temp.path().join("dist"), "/");
    let expected = vec![
        "boot.js".to_string(),
        "c/index.js".to_string(),
        "index.css".to_string(),
        "index.html".to_string(),
        "manifest.json".to_string(),
        "static-artifact-fixture.mjs".to_string(),
        "static-artifact-fixture.wasm".to_string(),
        "static-artifact-fixture.wat".to_string(),
    ];
    assert_eq!(files, expected, "静态产物树应为页面 + 成对浏览器产物，实际：{files:?}");
}

#[test]
fn static_page_references_resolve_to_emitted_artifacts() {
    // 页面内每个 `src`/`href` 都必须解析到产物树里的真实文件：这是页面↔产物的一致性证据。
    let temp = TempDir::new().expect("创建临时目录失败");
    let dist = temp.path().join("dist");
    build_and_list(temp.path(), &dist, "/");
    let html = fs::read_to_string(dist.join("index.html")).expect("读取 index.html 失败");
    assert!(html.contains("<title>产物树测试</title>"), "标题应写入页面:\n{html}");
    assert!(html.contains("静态产物树"), "根组件初始状态应写入页面:\n{html}");
    for attribute in ["src=\"", "href=\""] {
        for fragment in html.split(attribute).skip(1) {
            let reference = fragment.split('"').next().expect("引用值应以引号闭合");
            assert!(dist.join(reference).is_file(), "页面引用 {reference:?} 未解析到产物文件:\n{html}");
        }
    }
    assert!(html.contains("data-component=\"index\""), "页面应声明岛挂载点:\n{html}");
}

#[test]
fn manifest_references_resolve_to_emitted_artifacts() {
    // manifest 是 loader 的唯一入口：它声明的 wasm/glue/css/组件胶水必须全部存在，
    // 且组件名 ↔ 胶水路径 ↔ 页面挂载点三者对应。
    let temp = TempDir::new().expect("创建临时目录失败");
    let dist = temp.path().join("dist");
    build_and_list(temp.path(), &dist, "/");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dist.join("manifest.json")).expect("读取 manifest 失败")).expect("manifest 必须是合法 JSON");

    let wasm_entries = manifest.get("wasm").and_then(serde_json::Value::as_array).expect("manifest 应声明 wasm 条目");
    assert_eq!(wasm_entries.len(), 1, "静态交付应恰有一个 wasm chunk");
    let wasm_entry = &wasm_entries[0];
    for key in ["url", "glue"] {
        let reference = wasm_entry.get(key).and_then(serde_json::Value::as_str).expect("wasm 条目应声明 url/glue");
        assert!(dist.join(reference).is_file(), "manifest wasm.{key}={reference:?} 未解析到产物文件");
    }

    // hash：条目声明的摘要必须是真实 chunk 字节的 sha256，不是占位符。
    let declared_hash = wasm_entry.get("hash").and_then(serde_json::Value::as_str).expect("wasm 条目应声明 hash");
    let wasm_bytes = std::fs::read(dist.join(wasm_entry.get("url").and_then(serde_json::Value::as_str).unwrap())).expect("读取 wasm");
    let expected_hash =
        format!("sha256:{}", <sha2::Sha256 as sha2::Digest>::digest(&wasm_bytes).iter().map(|b| format!("{b:02x}")).collect::<String>());
    assert_eq!(declared_hash, expected_hash, "manifest hash 必须与落盘 chunk 字节对应");

    for css in manifest.get("css").and_then(serde_json::Value::as_array).expect("manifest 应声明 css") {
        let reference = css.as_str().expect("css 条目应为字符串");
        assert!(dist.join(reference).is_file(), "manifest css={reference:?} 未解析到产物文件");
    }

    let components = manifest.get("components").and_then(serde_json::Value::as_array).expect("manifest 应声明组件");
    assert_eq!(components.len(), 1, "静态交付应恰有一个组件胶水");
    let glue = components[0].get("js").and_then(serde_json::Value::as_str).expect("组件应声明 js");
    assert_eq!(components[0].get("name").and_then(serde_json::Value::as_str), Some("index"), "组件名应与入口 stem 对应");
    assert!(dist.join(glue).is_file(), "manifest 组件胶水 {glue:?} 未解析到产物文件");
}

#[test]
fn critical_css_is_inlined_and_registered_as_chunk() {
    // 首屏关键样式内联为页面 `<style>`，不再经 `<link>` 重复装载；
    // 同一份样式仍作为组件级 css chunk 落盘并由 manifest 登记（含组件关联）。
    let temp = TempDir::new().expect("创建临时目录失败");
    let dist = temp.path().join("dist");
    build_and_list(temp.path(), &dist, "/");
    let html = fs::read_to_string(dist.join("index.html")).expect("读取 index.html 失败");
    assert!(html.contains("<style>"), "入口组件关键样式应内联为首屏 <style>:\n{html}");
    assert!(html.contains(".sc-root { color: #94a3b8; }"), "内联样式必须是入口组件的合同降级:\n{html}");
    assert!(!html.contains("rel=\"stylesheet\""), "关键样式已内联，页面不得再链接装载:\n{html}");

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dist.join("manifest.json")).expect("读取 manifest 失败")).expect("manifest 必须是合法 JSON");
    let css = manifest.get("css").and_then(serde_json::Value::as_array).expect("manifest 应声明 css chunk");
    assert_eq!(css.len(), 1, "静态交付应恰有一个组件级 css chunk");
    assert_eq!(css[0].as_str(), Some("index.css"), "css chunk 必须按组件名登记");
    let components = manifest.get("components").and_then(serde_json::Value::as_array).expect("manifest 应声明组件");
    assert_eq!(
        components[0].get("css").and_then(serde_json::Value::as_str),
        Some("index.css"),
        "组件条目必须登记自己的 css chunk"
    );
}

#[test]
fn manifest_integrity_digests_match_emitted_artifact_bytes() {
    // 摘要必须由真实产物字节计算，而不是按文件名或构建版本推算：
    // 逐条读取 manifest `integrity`，重算 SHA-256 并与文件内容比对。
    let temp = TempDir::new().expect("创建临时目录失败");
    let dist = temp.path().join("dist");
    build_and_list(temp.path(), &dist, "/");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dist.join("manifest.json")).expect("读取 manifest 失败")).expect("manifest 必须是合法 JSON");
    let integrity = manifest.get("integrity").and_then(serde_json::Value::as_object).expect("manifest 必须声明 `integrity` 摘要表");
    assert!(!integrity.is_empty(), "摘要表不能为空");

    let required = ["boot.js", "c/index.js", "index.css", "static-artifact-fixture.mjs", "static-artifact-fixture.wasm"];
    for path in required {
        let expected = integrity.get(path).and_then(serde_json::Value::as_str).unwrap_or_else(|| panic!("manifest 摘要必须覆盖 {path}"));
        let bytes = fs::read(dist.join(path)).unwrap_or_else(|error| panic!("摘要引用 {path} 未解析到产物文件: {error}"));
        assert_eq!(sha256_hex(&bytes), expected, "{path} 的 manifest 摘要必须等于真实产物字节的 SHA-256");
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn obj(fields: Vec<(&str, ConfigValue)>) -> ConfigValue {
    ConfigValue::Object(fields.into_iter().map(|(name, value)| (name.to_owned(), value)).collect())
}

fn text(value: &str) -> ConfigValue {
    ConfigValue::Text(value.to_owned())
}

#[test]
fn resolved_base_is_consumed_by_static_build() {
    // 严格模型把 `artifact.base` 规范化为目录基址后，静态构建必须把它写入页面
    // `<base href>`，支持子目录部署；页面用相对 URL，因此产物路径随基址解析。
    let temp = TempDir::new().expect("创建临时目录失败");
    // 严格模型校验入口存在性；下面 build_and_list 会再写同一入口（幂等）。
    fs::create_dir_all(temp.path().join("source")).expect("创建 source/ 目录失败");
    fs::write(temp.path().join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    let value = obj(vec![(
        "artifact",
        obj(vec![
            ("entry", text("source/index.w")),
            ("base", text("/app/")),
            (
                "chunks",
                obj(vec![
                    ("boundaries", ConfigValue::List(vec![obj(vec![("component", text("Admin")), ("preload", ConfigValue::Bool(true))])])),
                    ("budgets", obj(vec![("entry_bytes", ConfigValue::Int(1024)), ("chunk_bytes", ConfigValue::Int(512))])),
                ]),
            ),
        ]),
    )]);
    let origin = ConfigOrigin { config_path: temp.path().join("asgard.config.v"), project_root: temp.path().to_path_buf() };
    let config = resolve_static_config(value, origin).expect("base/chunks 配置必须被严格模型接受");
    assert_eq!(config.base, "/app");
    assert_eq!(config.chunks.boundaries.len(), 1);

    build_and_list(temp.path(), &temp.path().join("dist"), &config.base);
    let html = fs::read_to_string(temp.path().join("dist/index.html")).expect("读取 index.html 失败");
    assert!(html.contains("<base href=\"/app/\">"), "构建必须消费规范化基址并写入 `<base href>`:\n{html}");
    // 子目录部署下页面仍引用相对产物路径（由 `<base href>` 解析到 `/app/` 下）。
    assert!(html.contains(r#"<script src="boot.js"></script>"#), "boot 引用应保持相对路径:\n{html}");
}
