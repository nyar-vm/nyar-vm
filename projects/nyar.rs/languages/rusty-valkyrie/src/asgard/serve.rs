//! 静态产物 HTTP 托管：MIME 表、路径解析与最小 GET/HEAD 服务。
//!
//! 本模块是「Wasm 用正确 MIME」的唯一事实所有者：产物目录里每个扩展名的响应头
//! 只在这里映射一次，构建侧与命令侧都不重复写表。服务只读产物目录，不解释页面、
//! 不做内容协商，也不改写响应体。
//!
//! 基址（`artifact.base`）的语义是部署前缀：设置 `/app/` 时只接受以该前缀开头的
//! 请求，去掉前缀后再映射到产物目录内文件；未设置时按根部署服务。

use std::{
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::asgard::error::{AsgardError, AsgardResult};

/// 静态托管选项。
#[derive(Debug, Clone)]
pub struct ServeOptions {
    /// 产物根目录（绝对或相对调用方工作目录）。
    pub root: PathBuf,
    /// 部署前缀（目录 URL，形如 `/` 或 `/app/`）。
    pub base: String,
    /// 监听主机；缺省 `127.0.0.1`。
    pub host: String,
    /// 监听端口；缺省 `3000`，`0` 表示由系统分配。
    pub port: u16,
}

impl ServeOptions {
    /// 缺省选项：本机 `127.0.0.1:3000`、根部署。
    pub fn for_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), base: "/".to_string(), host: "127.0.0.1".to_string(), port: 3000 }
    }
}

/// 一次静态托管运行的句柄。
pub struct ServeHandle {
    /// 实际监听地址（`port = 0` 时为系统分配的端口）。
    pub address: SocketAddr,
    /// 服务端线程；调用方负责停止进程或关闭监听。
    pub thread: std::thread::JoinHandle<()>,
}

/// 按扩展名给出响应 `Content-Type`。
///
/// 未知扩展名回落 `application/octet-stream`，不做猜测：错误 MIME 比缺失更危险
/// （浏览器会拒绝 `application/wasm` 之外的 chunk 响应）。
pub fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()).as_deref() {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") | Some("cjs") => "text/javascript; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("wat") => "text/plain; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// 启动静态托管；`on_ready` 在监听成功后收到实际地址（便于测试用 `port = 0`）。
///
/// 返回的线程持续接受请求直到进程退出；每个连接在独立线程中处理一次 `GET`/`HEAD`。
pub fn serve_static(options: &ServeOptions, on_ready: impl FnOnce(SocketAddr) + Send + 'static) -> AsgardResult<ServeHandle> {
    let root = std::path::absolute(&options.root)
        .map_err(|error| AsgardError::msg(report!("规范化托管根目录失败 {}: {error}", options.root.display())))?;
    let metadata = std::fs::metadata(&root).map_err(|error| AsgardError::msg(report!("托管根目录不可用 {}: {error}", root.display())))?;
    if !metadata.is_dir() {
        return Err(AsgardError::msg(report!("托管根不是目录: {}", root.display())));
    }
    let base = normalize_base(&options.base)?;
    let listener = TcpListener::bind((options.host.as_str(), options.port))
        .map_err(|error| AsgardError::msg(report!("监听 {}:{} 失败: {error}", options.host, options.port)))?;
    let address = listener.local_addr().map_err(|error| AsgardError::msg(report!("读取监听地址失败: {error}")))?;
    on_ready(address);

    let root = Arc::new(root);
    let thread = std::thread::Builder::new()
        .name("asgard-serve".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream
                else {
                    continue;
                };
                let root = Arc::clone(&root);
                let base = base.clone();
                std::thread::spawn(move || {
                    let _ = handle_connection(stream, &root, &base);
                });
            }
        })
        .map_err(|error| AsgardError::msg(report!("启动托管线程失败: {error}")))?;

    Ok(ServeHandle { address, thread })
}

/// 规范化部署前缀：空值视为 `/`，非 `/` 开头或含 `..` 段一律拒绝。
fn normalize_base(base: &str) -> AsgardResult<String> {
    let trimmed = base.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return Ok("/".to_string());
    }
    if !trimmed.starts_with('/') {
        return Err(AsgardError::msg(report!("托管基址必须以 `/` 开头: {trimmed}")));
    }
    if trimmed.split('/').any(|segment| segment == "..") {
        return Err(AsgardError::msg(report!("托管基址不得包含 `..` 段: {trimmed}")));
    }
    Ok(format!("{}/", trimmed.trim_end_matches('/')))
}

fn handle_connection(mut stream: TcpStream, root: &Path, base: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // 头部只读到空行为止，本服务不消费任何请求头。
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or_default();
    if method != "GET" && method != "HEAD" {
        return respond(&mut stream, 405, "text/plain; charset=utf-8", b"method not allowed\n", method == "HEAD");
    }

    let path = target.split(['?', '#']).next().unwrap_or("/");
    let Some(relative) = strip_base(path, base)
    else {
        return respond(&mut stream, 404, "text/plain; charset=utf-8", b"not found\n", method == "HEAD");
    };
    let Some(file_path) = resolve_path(root, &relative)
    else {
        return respond(&mut stream, 404, "text/plain; charset=utf-8", b"not found\n", method == "HEAD");
    };
    let bytes = match std::fs::read(&file_path) {
        Ok(bytes) => bytes,
        Err(_) => return respond(&mut stream, 404, "text/plain; charset=utf-8", b"not found\n", method == "HEAD"),
    };
    let content_type = content_type_for(&file_path);
    respond(&mut stream, 200, content_type, &bytes, method == "HEAD")
}

/// 去掉部署前缀；前缀不匹配返回 `None`。根部署（`/`）始终匹配。
fn strip_base<'a>(path: &'a str, base: &str) -> Option<&'a str> {
    if base == "/" {
        return Some(path.trim_start_matches('/'));
    }
    path.strip_prefix(base).map(|rest| rest.trim_start_matches('/'))
}

/// 把相对请求路径映射到产物内文件；`..` 段与越界一律拒绝。
fn resolve_path(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.split('/').any(|segment| segment == "..") {
        return None;
    }
    let candidate = if relative.is_empty() { root.join("index.html") } else { root.join(relative) };
    if candidate.is_dir() {
        let index = candidate.join("index.html");
        return index.is_file().then_some(index);
    }
    candidate.is_file().then_some(candidate)
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8], head_only: bool) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let headers = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}
