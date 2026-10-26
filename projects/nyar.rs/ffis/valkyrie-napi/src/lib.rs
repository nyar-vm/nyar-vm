//! Valkyrie 产品 Node-API 原生绑定：`legion` / `asgard` CLI 与程序化编译入口。
//!
//! 正交产品 API（`legionPlan` / `legionBuild` 等）由此 crate 导出；argv 巨型 CLI 由 TypeScript 重组。
//! `legionRun` / `runValkyrieCli` 仅为过渡透传，不再在 `rusty-valkyrie` 内使用 clap。

#![deny(missing_docs)]

use std::{ffi::OsString, process::ExitCode, thread};

use napi::bindgen_prelude::*;
use napi_derive::napi;
use rusty_valkyrie::legion::{SpyMode, SpyOptions, SpyTargetOptions, SpyTargetPlatform, cli, run_spy};

mod product_api;
mod stdio_capture;

pub use product_api::{LegionBuildOptions, LegionBuildOutcome, LegionPlanOptions, LegionPlanOutcome, LegionSemanticGroupItem};
pub use rusty_valkyrie::{asgard, legion};

/// `legion run` / `legion spy` 等 CLI 子进程等价结果。
#[napi(object)]
pub struct LegionRunOutcome {
    /// 进程退出码（0 表示成功）。
    pub status: u32,
    /// 捕获的标准输出。
    pub stdout: String,
    /// 捕获的标准错误。
    pub stderr: String,
}

/// `legion spy` 各模式共享的目标参数（与 Rust `SpyTargetOptions` 对齐）。
#[napi(object)]
pub struct SpyTargetRunOptions {
    /// 目标文件路径或项目名。
    pub input: Option<String>,
    /// 函数索引或函数名（wasm/lir/mir）。
    pub func: Option<String>,
    /// 方法名（jvm/clr）。
    pub method: Option<String>,
    /// 绝对偏移量（wasm）。
    pub offset: Option<i64>,
    /// 是否列出所有函数/方法。
    pub list: Option<bool>,
    /// 错误点上下文行数。
    pub context: Option<u32>,
    /// 编译目标：`wasm` / `jvm` / `clr`（verify / lir 模式）。
    pub target_platform: Option<String>,
    /// 是否以 JSON 格式输出。
    pub json: Option<bool>,
    /// 是否 dump 函数体原始字节（wasm）。
    pub hex: Option<bool>,
    /// 是否结构化解析 Type 段（wasm）。
    pub types: Option<bool>,
    /// 是否审计 wasm-gc struct/array 类型覆盖。
    pub gc_audit: Option<bool>,
    /// 是否审计 Node JS-glue 契约。
    pub glue_audit: Option<bool>,
}

/// 运行 Legion CLI（stdout/stderr 写入 Node 进程终端，不捕获）。
///
/// 新宿主请优先使用 [`legion_plan`] / [`legion_build`]；本函数保留给旧脚本。
#[napi]
pub fn legion_run(argv: Vec<String>) -> i32 {
    cli::run_with_argv(argv.into_iter().map(OsString::from).collect())
}

/// 运行 Legion CLI 并捕获 stdout/stderr。
///
/// 新宿主请优先使用 [`legion_plan`] / [`legion_build`]；本函数保留给旧脚本。
#[napi]
pub fn legion_run_captured(argv: Vec<String>) -> LegionRunOutcome {
    let os_argv: Vec<OsString> = argv.into_iter().map(OsString::from).collect();
    let (status, stdout, stderr) = thread::Builder::new()
        .name("legion-captured".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || stdio_capture::capture(|| cli::run_with_argv_in_current_thread(os_argv)))
        .expect("failed to spawn legion captured thread")
        .join()
        .expect("legion captured thread panicked");
    LegionRunOutcome { status: status as u32, stdout, stderr }
}

/// 运行 `legion spy <mode>` 并捕获输出。
#[napi]
pub fn legion_spy_run(mode: String, options: SpyTargetRunOptions) -> Result<LegionRunOutcome> {
    let spy_opts = map_spy_options(&mode, options)?;
    let (status, stdout, stderr) = thread::Builder::new()
        .name("legion-spy".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            stdio_capture::capture(|| match run_spy(&spy_opts) {
                Ok(code) => exit_code_to_i32(code),
                Err(report) => {
                    eprintln!("{report:?}");
                    1
                }
            })
        })
        .expect("failed to spawn legion spy thread")
        .join()
        .expect("legion spy thread panicked");
    Ok(LegionRunOutcome { status: status as u32, stdout, stderr })
}

/// 解析 argv 但不执行（仅供旧脚本；新宿主请用 [`legion_plan`] / [`legion_build`]）。
#[napi]
pub fn legion_parse(_argv: Vec<String>) -> Result<String> {
    Err(Error::from_reason("legionParse 已废弃：请在 TypeScript 侧解析 argv，并调用 legionPlan / legionBuild 等正交 N-API。"))
}

/// 运行 Valkyrie 编译器 CLI（`legion` / `asgard`），返回进程退出码。
///
/// `product` 仅接受 `"legion"` 与 `"asgard"`，调用时补入程序名后把 `argv` 原样
/// 转交 Rust CLI；不在此处解析或补全命令语义。
#[napi]
pub fn run_valkyrie_cli(product: String, argv: Vec<String>) -> Result<i32> {
    let mut os_argv: Vec<OsString> = Vec::with_capacity(argv.len() + 1);
    os_argv.push(OsString::from(product.as_str()));
    os_argv.extend(argv.into_iter().map(OsString::from));
    match product.as_str() {
        "legion" => Ok(rusty_valkyrie::legion::cli::run_with_argv(os_argv)),
        "asgard" => Ok(rusty_valkyrie::asgard::cli::run_with_argv(os_argv)),
        other => Err(Error::from_reason(format!("unsupported valkyrie product '{other}' (expected legion/asgard)"))),
    }
}

fn exit_code_to_i32(code: ExitCode) -> i32 {
    if code == ExitCode::SUCCESS { 0 } else { 1 }
}

fn map_spy_options(mode: &str, options: SpyTargetRunOptions) -> Result<SpyOptions> {
    let target = SpyTargetOptions {
        input: options.input,
        func: options.func,
        method: options.method,
        offset: options.offset,
        list: options.list.unwrap_or(false),
        context: options.context.unwrap_or(20) as usize,
        target_platform: options.target_platform.as_deref().map(parse_spy_target_platform).transpose()?,
        json: options.json.unwrap_or(false),
        hex: options.hex.unwrap_or(false),
        types: options.types.unwrap_or(false),
        gc_audit: options.gc_audit.unwrap_or(false),
        glue_audit: options.glue_audit.unwrap_or(false),
    };

    let mode = match mode.trim().to_ascii_lowercase().as_str() {
        "wasm" => SpyMode::Wasm(target),
        "jvm" => SpyMode::Jvm(target),
        "clr" => SpyMode::Clr(target),
        "native" => SpyMode::Native(target),
        "lir" => SpyMode::Lir(target),
        "mir" => SpyMode::Mir(target),
        "verify" => SpyMode::Verify(target),
        other => {
            return Err(Error::from_reason(format!("unsupported spy mode '{other}' (expected wasm/jvm/clr/native/lir/mir/verify)")));
        }
    };

    Ok(SpyOptions { mode })
}

fn parse_spy_target_platform(value: &str) -> Result<SpyTargetPlatform> {
    match value.trim().to_ascii_lowercase().as_str() {
        "wasm" => Ok(SpyTargetPlatform::Wasm),
        "jvm" => Ok(SpyTargetPlatform::Jvm),
        "clr" => Ok(SpyTargetPlatform::Clr),
        other => Err(Error::from_reason(format!("unsupported spy target platform '{other}' (expected wasm/jvm/clr)"))),
    }
}
