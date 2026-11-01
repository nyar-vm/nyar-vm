//! `asgard::serve` 合同：MIME 表、路径映射与真实 HTTP 响应（根部署与 base 子目录）。

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::Path,
    sync::mpsc,
};

use rusty_valkyrie::asgard::serve::{ServeOptions, content_type_for, serve_static};
use tempfile::TempDir;

/// 一次原始 HTTP/1.1 GET/HEAD 的结果：状态行、头部与响应体文本。
struct HttpReply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl HttpReply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }
}

fn request(address: std::net::SocketAddr, method: &str, path: &str) -> HttpReply {
    let mut stream = TcpStream::connect(address).expect("连接托管服务失败");
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: asgard.test\r\nConnection: close\r\n\r\n").expect("写入请求失败");
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
    let mut body = String::new();
    std::io::Read::read_to_string(&mut reader, &mut body).expect("读取响应体失败");
    HttpReply { status, headers, body }
}

/// 写入一个最小产物目录；返回值用于断言响应体。
fn write_dist(temp: &TempDir) -> std::path::PathBuf {
    let dist = temp.path().join("dist");
    fs::create_dir_all(&dist).expect("创建产物目录失败");
    fs::write(dist.join("index.html"), "<!DOCTYPE html><title>serve</title>").expect("写入页面失败");
    fs::write(dist.join("manifest.json"), "{\"module\":\"serve\"}").expect("写入 manifest 失败");
    fs::write(dist.join("app.wasm"), b"\0asm\x01\0\0\0").expect("写入 chunk 失败");
    fs::write(dist.join("app.css"), ".hero{color:#333}").expect("写入样式失败");
    fs::write(temp.path().join("secret.txt"), "outside").expect("写入越界文件失败");
    dist
}

/// 启动托管并返回实际地址；服务线程在测试进程退出时随之结束。
fn start(root: &Path, base: &str) -> std::net::SocketAddr {
    let (sender, receiver) = mpsc::channel();
    serve_static(&ServeOptions { root: root.to_path_buf(), base: base.to_string(), host: "127.0.0.1".to_string(), port: 0 }, move |address| {
        let _ = sender.send(address);
    })
    .expect("托管启动应成功");
    receiver.recv().expect("应收到监听地址")
}

#[test]
fn content_type_table_covers_delivery_artifacts() {
    assert_eq!(content_type_for(Path::new("app.wasm")), "application/wasm");
    assert_eq!(content_type_for(Path::new("c/index.js")), "text/javascript; charset=utf-8");
    assert_eq!(content_type_for(Path::new("app.mjs")), "text/javascript; charset=utf-8");
    assert_eq!(content_type_for(Path::new("manifest.json")), "application/json; charset=utf-8");
    assert_eq!(content_type_for(Path::new("index.html")), "text/html; charset=utf-8");
    assert_eq!(content_type_for(Path::new("app.css")), "text/css; charset=utf-8");
    assert_eq!(content_type_for(Path::new("app.wat")), "text/plain; charset=utf-8");
    assert_eq!(content_type_for(Path::new("blob.bin")), "application/octet-stream");
}

#[test]
fn root_deployment_serves_artifacts_with_real_mime() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let dist = write_dist(&temp);
    let address = start(&dist, "/");

    let page = request(address, "GET", "/");
    assert_eq!(page.status, 200, "根路径应返回页面");
    assert!(page.body.contains("<title>serve</title>"), "根路径应返回 index.html 内容");
    assert_eq!(page.header("content-type"), Some("text/html; charset=utf-8"));

    let wasm = request(address, "GET", "/app.wasm");
    assert_eq!(wasm.status, 200);
    assert_eq!(wasm.header("content-type"), Some("application/wasm"), "Wasm 必须用 application/wasm");

    let manifest = request(address, "GET", "/manifest.json");
    assert_eq!(manifest.header("content-type"), Some("application/json; charset=utf-8"));

    let head = request(address, "HEAD", "/app.wasm");
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty(), "HEAD 不得返回响应体");

    let missing = request(address, "GET", "/missing.wasm");
    assert_eq!(missing.status, 404);

    let traversal = request(address, "GET", "/../secret.txt");
    assert_eq!(traversal.status, 404, "越界路径必须拒绝");
}

#[test]
fn base_prefix_serves_subdirectory_deployment() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let dist = write_dist(&temp);
    let address = start(&dist, "/app");

    let page = request(address, "GET", "/app/");
    assert_eq!(page.status, 200, "带前缀的根路径应返回页面");
    assert_eq!(page.header("content-type"), Some("text/html; charset=utf-8"));

    let wasm = request(address, "GET", "/app/app.wasm");
    assert_eq!(wasm.status, 200);
    assert_eq!(wasm.header("content-type"), Some("application/wasm"));

    let unprefixed = request(address, "GET", "/app.wasm");
    assert_eq!(unprefixed.status, 404, "未带部署前缀的请求必须拒绝");
}
