//! 真实构建产物经产品 serve 路径（`asgard::serve::serve_static`）的端到端验证。
//!
//! 不用合成目录充当产物：先跑真实 `build_static_site`，再按 `manifest.json`
//! 声明的 URL 逐一发真实 HTTP 请求，断言 MIME 与 `integrity` 摘要逐字节对应。
//! base 子目录变体断言部署前缀下的可达性与 `<base href>` 加载合同。

use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    sync::mpsc,
};

use rusty_valkyrie::asgard::{
    serve::{ServeOptions, serve_static},
    static_site::{StaticSiteOptions, build_static_site},
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::support::write_legion_project;

/// 带交互状态的根组件入口；widget 名与入口文件 stem（`index`）一致。
const FIXTURE: &str = r#"<widget index>
<Column class="hero">
    <Text class="hero-title">{greeting}</Text>
</Column>
</widget>
<script>
let greeting: utf8 = "serve 真实产物"
</script>
<style>
root { color: @brand }
</style>
"#;

/// 一次原始 HTTP/1.1 GET 的结果：状态码、头部与响应体字节。
struct HttpReply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpReply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// 响应体的小写十六进制 SHA-256。
    fn sha256_hex(&self) -> String {
        Sha256::digest(&self.body).iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

fn get(address: std::net::SocketAddr, path: &str) -> HttpReply {
    let mut stream = TcpStream::connect(address).expect("连接托管服务失败");
    write!(stream, "GET {path} HTTP/1.1\r\nHost: asgard.test\r\nConnection: close\r\n\r\n").expect("写入请求失败");
    stream.flush().expect("刷新请求失败");
    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line).expect("读取状态行失败");
    let status = status_line.split_whitespace().nth(1).and_then(|code| code.parse().ok()).unwrap_or(0);
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).expect("读取头部失败") == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let mut body = Vec::new();
    reader.read_to_end(&mut body).unwrap_or_else(|error| panic!("读取响应体失败 kind={:?}: {error}", error.kind()));
    HttpReply { status, headers, body }
}

/// 构建真实静态站点并启动托管，返回（临时目录、产物目录、监听地址）。
fn build_and_serve(base: &str) -> (TempDir, PathBuf, std::net::SocketAddr) {
    let temp = TempDir::new().expect("创建临时目录失败");
    let project = temp.path();
    fs::create_dir_all(project.join("source")).expect("创建 source/ 目录失败");
    write_legion_project(project, "serve-static-build");
    fs::write(project.join("source/index.w"), FIXTURE).expect("写入入口文件失败");
    let report = build_static_site(&StaticSiteOptions {
        project_dir: project.to_path_buf(),
        entry: PathBuf::from("source/index.w"),
        output_dir: None,
        title: None,
        base: base.into(),
        assets_dir: None,
    })
    .expect("真实静态构建应成功");

    let (sender, receiver) = mpsc::channel();
    let root = report.output_dir.clone();
    serve_static(&ServeOptions { root: root.clone(), base: base.to_string(), host: "127.0.0.1".to_string(), port: 0 }, move |address| {
        let _ = sender.send(address);
    })
    .expect("托管启动应成功");
    let address = receiver.recv().expect("应收到监听地址");
    (temp, root, address)
}

/// 读取 manifest 并返回（wasm URL、glue URL、组件 JS 列表、CSS 列表、integrity 表）。
fn read_manifest(dist: &Path) -> (String, String, Vec<String>, Vec<String>, serde_json::Map<String, serde_json::Value>) {
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dist.join("manifest.json")).expect("读取 manifest 失败")).expect("manifest 必须是合法 JSON");
    let wasm_url = manifest["wasm"][0]["url"].as_str().expect("manifest 必须声明 wasm URL").to_string();
    let glue_url = manifest["wasm"][0]["glue"].as_str().expect("manifest 必须声明 glue URL").to_string();
    let components = manifest["components"].as_array().expect("components 必须是数组");
    let component_js = components.iter().map(|entry| entry["js"].as_str().expect("组件条目必须有 js").to_string()).collect();
    let css = manifest["css"]
        .as_array()
        .expect("css 必须是数组")
        .iter()
        .map(|entry| entry.as_str().expect("css 条目必须是文本").to_string())
        .collect();
    let integrity = manifest["integrity"].as_object().expect("integrity 必须是对象").clone();
    (wasm_url, glue_url, component_js, css, integrity)
}

/// 请求一个 manifest 声明的产物，断言状态、MIME 与 integrity 摘要逐字节对应。
fn assert_artifact(address: std::net::SocketAddr, prefix: &str, url: &str, mime: &str, integrity: &serde_json::Map<String, serde_json::Value>) {
    let reply = get(address, &format!("{prefix}{url}"));
    assert_eq!(reply.status, 200, "{url} 必须可服务");
    assert_eq!(reply.header("content-type"), Some(mime), "{url} 的 MIME 必须是 {mime}");
    let expected = integrity.get(url).and_then(|value| value.as_str()).unwrap_or_else(|| panic!("integrity 必须覆盖 {url}"));
    assert_eq!(&reply.sha256_hex(), expected, "{url} 的服务字节必须与 integrity 摘要一致");
}

#[test]
fn served_build_artifacts_match_manifest_mime_and_integrity() {
    let (_temp, dist, address) = build_and_serve("/");
    let (wasm_url, glue_url, component_js, css, integrity) = read_manifest(&dist);

    let page = get(address, "/");
    assert_eq!(page.status, 200);
    assert_eq!(page.header("content-type"), Some("text/html; charset=utf-8"));

    let manifest_reply = get(address, "/manifest.json");
    assert_eq!(manifest_reply.header("content-type"), Some("application/json; charset=utf-8"));

    // Wasm chunk 与其 glue：MIME 正确且字节与 manifest 摘要对应。
    assert_artifact(address, "/", &wasm_url, "application/wasm", &integrity);
    assert_artifact(address, "/", &glue_url, "text/javascript; charset=utf-8", &integrity);
    // chunk 条目 hash 与 integrity 表同一摘要来源。
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dist.join("manifest.json")).expect("读取 manifest 失败")).expect("manifest 必须是合法 JSON");
    let chunk_hash = manifest["wasm"][0]["hash"].as_str().expect("wasm 条目必须带 hash");
    assert_eq!(
        chunk_hash,
        format!("sha256:{}", integrity[&wasm_url].as_str().expect("integrity 值必须是文本")),
        "chunk hash 必须出自 integrity 同一摘要"
    );

    // 组件胶水、boot 加载器与样式逐一经真实请求验证。
    assert!(!component_js.is_empty(), "静态交付必须产出组件胶水");
    for js in &component_js {
        assert_artifact(address, "/", js, "text/javascript; charset=utf-8", &integrity);
    }
    assert_artifact(address, "/", "boot.js", "text/javascript; charset=utf-8", &integrity);
    for css_url in &css {
        assert_artifact(address, "/", css_url, "text/css; charset=utf-8", &integrity);
    }
}

#[test]
fn base_prefix_serves_real_build_under_the_deployment_prefix() {
    let (_temp, dist, address) = build_and_serve("/app");
    let (wasm_url, glue_url, component_js, _css, integrity) = read_manifest(&dist);

    let page = get(address, "/app/");
    assert_eq!(page.status, 200, "带前缀的根路径必须返回页面");
    assert_eq!(page.header("content-type"), Some("text/html; charset=utf-8"));
    // 加载合同：页面声明 `<base href>`，loader 的相对产物 URL 在部署前缀下解析。
    let page_text = String::from_utf8(page.body).expect("页面必须是 UTF-8");
    assert!(page_text.contains("<base href=\"/app/\">"), "非根基址必须写入 `<base href>`:\n{page_text}");

    let manifest_reply = get(address, "/app/manifest.json");
    assert_eq!(manifest_reply.status, 200);
    assert_eq!(manifest_reply.header("content-type"), Some("application/json; charset=utf-8"));

    assert_artifact(address, "/app/", &wasm_url, "application/wasm", &integrity);
    assert_artifact(address, "/app/", &glue_url, "text/javascript; charset=utf-8", &integrity);
    for js in &component_js {
        assert_artifact(address, "/app/", js, "text/javascript; charset=utf-8", &integrity);
    }

    // 未带部署前缀的请求不得命中产物。
    let unprefixed = get(address, &format!("/{wasm_url}"));
    assert_eq!(unprefixed.status, 404, "未带部署前缀的请求必须拒绝");
}
