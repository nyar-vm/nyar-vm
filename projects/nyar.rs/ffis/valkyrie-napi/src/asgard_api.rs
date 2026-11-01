//! Asgard 程序化 N-API 产品面（`asgardBuildStatic`），不经 argv CLI。
//!
//! 与 `legionPlan` / `legionBuild` 同型：TypeScript 侧解析 argv 后调用正交 API，
//! 本层只做参数搬运与结构化结果映射，静态构建的唯一事实所有者是
//! `rusty_valkyrie::asgard::static_site`。
//!
//! 准入与命令面共用 `rusty_valkyrie::asgard::cli::require_project_admission`：
//! 缺 `legion.von` 或 `asgard.config.v` 的项目在这里被拒绝，不因为走 N-API 而绕过。

use std::{path::PathBuf, thread};

use napi_derive::napi;
use rusty_valkyrie::asgard::{
    cli::require_project_admission,
    static_site::{StaticSiteOptions, build_static_site},
};

/// `asgardBuildStatic` 入参。
#[napi(object)]
pub struct AsgardStaticBuildOptions {
    /// 项目根目录。
    pub project_dir: String,
    /// 入口根组件文件路径（相对 `projectDir`，`.awsl` 规范拼写或同义缩写 `.w`，须显式给出）。
    pub entry: String,
    /// 输出目录；缺省为 `projectDir/dist`。
    pub output_dir: Option<String>,
    /// 页面标题；缺省为根组件名（入口文件 stem）。
    pub title: Option<String>,
    /// 产物基址（目录 URL）；缺省 `/`。配置求值闭合前由调用方显式给出，本层不猜测。
    pub base: Option<String>,
    /// 显式资源目录（相对 `projectDir`）；缺省不拷贝资源。
    pub assets_dir: Option<String>,
}

/// `asgardBuildStatic` 的结构化返回。
#[napi(object)]
pub struct AsgardStaticBuildOutcome {
    /// 是否成功。
    pub ok: bool,
    /// 失败时的错误描述。
    pub error: Option<String>,
    /// 实际写出产物的目录。
    pub output_dir: Option<String>,
    /// 产物 `index.html` 的完整路径。
    pub page_path: Option<String>,
    /// 根组件名。
    pub component_name: Option<String>,
}

/// `asgardServe` 入参。
#[napi(object)]
pub struct AsgardServeOptions {
    /// 产物根目录。
    pub root: String,
    /// 部署前缀（目录 URL）；缺省 `/`。
    pub base: Option<String>,
    /// 监听主机；缺省 `127.0.0.1`。
    pub host: Option<String>,
    /// 监听端口；缺省 `3000`，`0` 表示系统分配。
    pub port: Option<u32>,
}

/// `asgardServe` 的结构化返回；成功时进程内已启动服务线程，本调用阻塞直到进程退出。
#[napi(object)]
pub struct AsgardServeOutcome {
    /// 是否成功启动。
    pub ok: bool,
    /// 失败时的错误描述。
    pub error: Option<String>,
    /// 实际监听地址（`host:port`）。
    pub address: Option<String>,
}

/// 托管静态产物（`application/wasm` 等 MIME 由 `asgard::serve` 单一拥有）。
#[napi]
pub fn asgard_serve(options: AsgardServeOptions) -> AsgardServeOutcome {
    let serve_options = rusty_valkyrie::asgard::serve::ServeOptions {
        root: PathBuf::from(&options.root),
        base: options.base.unwrap_or_else(|| "/".to_string()),
        host: options.host.unwrap_or_else(|| "127.0.0.1".to_string()),
        port: u16::try_from(options.port.unwrap_or(3000)).unwrap_or(3000),
    };
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let handle = match rusty_valkyrie::asgard::serve::serve_static(&serve_options, move |address| {
        let _ = ready_sender.send(address);
    }) {
        Ok(handle) => handle,
        Err(error) => return AsgardServeOutcome { ok: false, error: Some(error.to_string()), address: None },
    };
    let address = ready_receiver.recv().map(|address| address.to_string()).unwrap_or_default();
    let _ = handle.thread.join();
    AsgardServeOutcome { ok: true, error: None, address: Some(address) }
}

/// 构建静态站点（页面 + WASM chunk + 岛包产物）并返回结构化结果。
#[napi]
pub fn asgard_build_static(options: AsgardStaticBuildOptions) -> AsgardStaticBuildOutcome {
    let worker = thread::Builder::new()
        .name("asgard-static-build".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let project_dir = PathBuf::from(&options.project_dir);
            require_project_admission(&project_dir)?;
            build_static_site(&StaticSiteOptions {
                project_dir,
                entry: PathBuf::from(&options.entry),
                output_dir: options.output_dir.map(PathBuf::from),
                title: options.title,
                base: options.base.unwrap_or_else(|| "/".to_string()),
                assets_dir: options.assets_dir.map(PathBuf::from),
            })
        })
        .expect("failed to spawn asgard static build thread");
    match worker.join().expect("asgard static build thread panicked") {
        Ok(report) => AsgardStaticBuildOutcome {
            ok: true,
            error: None,
            output_dir: Some(report.output_dir.display().to_string()),
            page_path: Some(report.page_path.display().to_string()),
            component_name: Some(report.component_name),
        },
        Err(error) => {
            AsgardStaticBuildOutcome { ok: false, error: Some(error.to_string()), output_dir: None, page_path: None, component_name: None }
        }
    }
}
