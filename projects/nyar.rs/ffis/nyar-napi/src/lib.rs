//! Node-API 原生绑定：导出正交、粗粒度的宿主能力（协议、运行时家族、命令准备/执行），
//! 以及 Valkyrie 编译器 CLI（`legion` / `asgard`）的单一原生入口。
//!
//! CLI argv 解析与命令分发由 Rust 侧 `legion::cli` / `asgard::cli` 承担；
//! 本 crate 只透传 argv 与退出码，不复制 CLI 语义。

#![deny(missing_docs)]

use std::{collections::HashMap, ffi::OsString, path::Path, process::ExitCode, thread};

use rusty_valkyrie::legion::{SpyMode, SpyOptions, SpyTargetOptions, SpyTargetPlatform, cli, run_spy};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use nyar_runner::{PreparedRuntimeCommand, RuntimeContract, RuntimeFamily};

mod host_api;
mod stdio_capture;

pub use rusty_valkyrie::{asgard, legion};
pub use host_api::{LegionBuildOptions, LegionBuildOutcome, LegionPlanOptions, LegionPlanOutcome, LegionSemanticGroupItem};

/// 当前 Node 宿主协议 id（与 JS `HOST_PROTOCOL` 对齐）。
pub const HOST_PROTOCOL: &str = "nyar-host/0";

/// 协议与宿主元信息。
#[napi(object)]
pub struct JsHostInfo {
    /// 宿主协议 id。
    pub host_protocol: String,
    /// 绑定 crate 名。
    pub binding: String,
}

/// 准备运行时命令的输入。
#[napi(object)]
pub struct JsPrepareRuntimeOptions {
    /// 运行时家族 id（`clr` / `jvm` / `node` / `wasi` / `nyar-vm` 等）。
    pub family: String,
    /// 产物路径（artifact / jar / wasm 等）。
    pub artifact: String,
    /// JVM classpath；其他家族可留空。
    pub classpath: Option<String>,
    /// 逻辑入口名。
    pub entry: String,
    /// WASI 家族是否启用 `-S p3`。
    pub wasi_p3: Option<bool>,
}

/// 已展开占位符、可直接执行的运行命令。
#[napi(object)]
pub struct JsPreparedRuntimeCommand {
    /// 运行时家族 id。
    pub family: String,
    /// 可执行文件或解释器。
    pub command: String,
    /// 参数列表。
    pub args: Vec<String>,
}

fn parse_family(value: &str) -> Result<RuntimeFamily> {
    RuntimeFamily::parse(value).ok_or_else(|| Error::from_reason(format!("unsupported runtime family '{}'", value)))
}

fn to_js_prepared(cmd: PreparedRuntimeCommand) -> JsPreparedRuntimeCommand {
    JsPreparedRuntimeCommand { family: cmd.family.as_str().to_string(), command: cmd.command, args: cmd.args }
}

/// 返回宿主协议与绑定元信息。
#[napi]
pub fn get_host_info() -> JsHostInfo {
    JsHostInfo { host_protocol: HOST_PROTOCOL.to_string(), binding: "nyar-napi".to_string() }
}

/// 列出支持的运行时家族 id。
#[napi]
pub fn list_runtime_families() -> Vec<String> {
    [RuntimeFamily::Clr, RuntimeFamily::Jvm, RuntimeFamily::Node, RuntimeFamily::Windows, RuntimeFamily::Wasi, RuntimeFamily::Ovm]
        .into_iter()
        .map(|family| family.as_str().to_string())
        .collect()
}

/// 解析运行时家族 id；非法输入返回 Error。
#[napi]
pub fn parse_runtime_family(value: String) -> Result<String> {
    Ok(parse_family(&value)?.as_str().to_string())
}

/// 按家族模板展开占位符，返回可执行命令（不启动进程）。
#[napi]
pub fn prepare_runtime_command(options: JsPrepareRuntimeOptions) -> Result<JsPreparedRuntimeCommand> {
    let family = parse_family(&options.family)?;
    let contract =
        RuntimeContract { logical_entry: Some(options.entry.as_str()), physical_entry: None, wasi_p3: options.wasi_p3.unwrap_or(false) };
    let template = family.default_template(Some(contract));
    let classpath = options.classpath.as_deref().unwrap_or(".");
    let prepared = template.prepare_command(Path::new(&options.artifact), Path::new(classpath), &options.entry);
    Ok(to_js_prepared(prepared))
}

/// 执行宿主脚本的输入。
#[napi(object)]
pub struct JsHostScriptOptions {
    /// 语言 id（`bash` / `sh` / `lua` / `tcl` 等）。
    pub language: String,
    /// 脚本源码。
    pub source: String,
    /// 脚本路径；仅用于诊断，可留空。
    pub file: Option<String>,
}

/// 宿主脚本执行结果。
#[napi(object)]
pub struct JsHostScriptResult {
    /// 脚本最终值的字符串表示。
    pub value: String,
}

/// 执行宿主脚本源码，返回脚本最终值。
#[napi]
pub fn run_host_script(options: JsHostScriptOptions) -> Result<JsHostScriptResult> {
    let runner = legend_vm::LegendVmRunner::new();
    let mut env = HashMap::new();
    let value = runner
        .run(&options.language, &options.source, &mut env)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    Ok(JsHostScriptResult { value: value.to_string_value() })
}

/// 在指定工作目录执行已准备好的命令并返回退出码。
#[napi]
pub fn run_prepared_command(prepared: JsPreparedRuntimeCommand, cwd: Option<String>) -> Result<i32> {
    let family = parse_family(&prepared.family)?;
    let cmd = PreparedRuntimeCommand { family, command: prepared.command, args: prepared.args };
    let cwd = cwd.as_deref().map(Path::new).unwrap_or_else(|| Path::new("."));
    Ok(cmd.run_in(cwd).map_err(|err| Error::from_reason(err.to_string()))?)
}

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
pub fn legion_parse(argv: Vec<String>) -> Result<String> {
    use clap::Parser;
    use rusty_valkyrie::legion::cli::LegionCli;

    let os_argv: Vec<OsString> = argv.into_iter().map(OsString::from).collect();
    if rusty_valkyrie::legion::forward::try_forward_companion(&os_argv).is_some() {
        return Ok("forward".to_string());
    }
    let cli = LegionCli::try_parse_from(&os_argv).map_err(|error| Error::from_reason(error.to_string()))?;
    Ok(format!("{cli:?}"))
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
