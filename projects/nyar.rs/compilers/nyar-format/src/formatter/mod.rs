//! LSP / CLI **格式化注册表**（契约见 `nyar_analyzer::format::SourceFormatterRegistry`）。
//!
//! 本模块只提供平台侧注册表与扩展名表：具体语言的 formatter 由前端 crate 在启动时调用
//! [`register_source_provider`] 注册（方向是前端 → 平台），平台不反向依赖任何前端。
//! 语言前端 provider 由前端装配入口注册；VON / AWSL 由本 crate 直接注册。合法 formatter
//! 只能来自上游 `oak-<language>/src/formatter/`（见 `oak-typescript::formatter`），
//! 不得在此新增 CST token-gap 排版规则。
//!
//! 与 [`crate::printer`]（AST pretty print）严格分离，对齐 Oak 的 `formatter/` vs `printer/`；
//! 早期把 printer API 与 deprecated `FormatBuffer` 从本模块重导出的混合接口已删除，
//! 消费者必须直接使用 `crate::printer`。
//!
//! - **SourceFormatter**：源码 → CST token-gap → 文本（保留 trivia）
//! - **配置**：默认从各文件路径的 `.editorconfig` 解析（[`FormatBatchOptions::use_editorconfig`]）

mod batch;
mod cli;

use std::{
    path::{Path, PathBuf},
    sync::{OnceLock, RwLock, RwLockReadGuard},
};

pub use batch::{FormatBatchOptions, FormatBatchReport, format_paths};
pub use cli::{
    FormatCliOptions, format_options_from_cli, normalize_extensions, report_format_cli, report_format_cli_with, run_format_cli,
    run_format_cli_with,
};
pub use nyar_analyzer::format::{
    ByteRange, Document, FormatConfigLoader, FormatConfigResolution, FormatError, FormatOptions, FormattedOutput, PrintStyle, Printer,
    PrinterProvider, PrinterRegistry, SourceFormatter, SourceFormatterProvider, SourceFormatterRegistry, SourceMap,
};

/// 核心源文件扩展名（前端 provider 负责语言语义，平台只拥有扩展名表）。
pub const V_EXTENSIONS: &[&str] = &["v", "valkyrie"];

/// 模板 + X-Grammar / widget 视图（`.vx`）。
pub const VX_EXTENSIONS: &[&str] = &["vx"];

/// VON 配置/清单。
pub const VON_EXTENSIONS: &[&str] = &["von"];

/// 模板语言面（与 `.v`/`.vx` 不同）。
pub const AWSL_EXTENSIONS: &[&str] = &["awsl"];

/// 平台可识别的可格式化源码种类（按文件扩展名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// 核心语言：`.v` / `.valkyrie`（formatter 由前端 provider 注册）。
    V,
    /// 模板 + X-Grammar：`.vx`（formatter 由前端 provider 注册）。
    Vx,
    /// VON 数据：`.von`（`oak-von::formatter`）。
    Von,
    /// AWSL 模板：`.awsl`（`oak-awsl::formatter`）。
    Awsl,
}

impl SourceKind {
    /// 对应 `SourceFormatter::language_id`。
    pub fn language_id(self) -> &'static str {
        match self {
            Self::V => "v",
            Self::Vx => "vx",
            Self::Von => "von",
            Self::Awsl => "awsl",
        }
    }

    /// 由文件扩展名推断种类。
    pub fn from_extension(ext: &str) -> Option<Self> {
        source_kind_from_extension(ext)
    }

    /// 由路径推断。
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension().and_then(|ext| ext.to_str()).and_then(Self::from_extension)
    }
}

/// 由扩展名推断 `SourceKind`（legacy `.vk` 不再识别）。
pub fn source_kind_from_extension(ext: &str) -> Option<SourceKind> {
    if V_EXTENSIONS.contains(&ext) {
        Some(SourceKind::V)
    }
    else if VX_EXTENSIONS.contains(&ext) {
        Some(SourceKind::Vx)
    }
    else if VON_EXTENSIONS.contains(&ext) {
        Some(SourceKind::Von)
    }
    else if AWSL_EXTENSIONS.contains(&ext) {
        Some(SourceKind::Awsl)
    }
    else {
        None
    }
}

/// 单文件格式化结果（不写盘）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatFileOutcome {
    /// 源文件路径。
    pub path: PathBuf,
    /// 格式化后的文本。
    pub formatted: String,
    /// 是否与原始内容不同。
    pub changed: bool,
}

struct VonSourceFormatter;
impl SourceFormatter for VonSourceFormatter {
    fn language_id(&self) -> &str {
        "von"
    }

    fn format_document(&self, source: &str, options: &FormatOptions) -> Result<FormattedOutput, FormatError> {
        crate::von::source_format::format_von_source(source, options)
    }
}

struct AwslSourceFormatter;
impl SourceFormatter for AwslSourceFormatter {
    fn language_id(&self) -> &str {
        "awsl"
    }

    fn format_document(&self, source: &str, options: &FormatOptions) -> Result<FormattedOutput, FormatError> {
        crate::awsl::source_format::format_awsl(source, options)
    }
}

/// 平台源码格式化注册表（启动时由各前端 provider 补全）。
///
/// 读取方应通过 [`format_source`] / [`format_source_range`] / [`format_path`] 间接访问，
/// 避免长时间持有写锁。`v` / `vx` 只有在对应前端 provider 注册后才可用；
/// 未注册时返回 [`FormatError::Unsupported`]。
///
/// 不要反向依赖具体前端；provider 由装配层（宿主 CLI、集成测试）注册。
pub fn source_registry() -> &'static RwLock<SourceFormatterRegistry> {
    static REGISTRY: OnceLock<RwLock<SourceFormatterRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut reg = SourceFormatterRegistry::new();
        reg.register(&["von"], Box::new(VonSourceFormatter));
        reg.register(&["awsl"], Box::new(AwslSourceFormatter));
        // JS/TS/JSON 与 Python 的源码排版归各自 Oak formatter（`oak-typescript::formatter` 等）；
        // 本 crate 不得再注册「只裁剪空白行」的第二套本地 formatter 冒充格式化。
        // 在这些 Oak formatter 接入前，相应扩展名按 unsupported 失败。
        RwLock::new(reg)
    })
}

/// 注册前端源码格式化 provider（线程安全，可重复调用；先注册者优先）。
pub fn register_source_provider(provider: &dyn SourceFormatterProvider) {
    source_registry().write().unwrap_or_else(|error| error.into_inner()).register_provider(provider);
}

fn source_registry_read() -> RwLockReadGuard<'static, SourceFormatterRegistry> {
    source_registry().read().unwrap_or_else(|error| error.into_inner())
}

/// 将源码格式化为规范文本。
pub fn format_source(kind: SourceKind, source: &str, options: &FormatOptions) -> Result<String, FormatError> {
    source_registry_read().format(kind.language_id(), source, options)
}

/// 格式化选中字节范围。
pub fn format_source_range(kind: SourceKind, source: &str, range: ByteRange, options: &FormatOptions) -> Result<FormattedOutput, FormatError> {
    source_registry_read().format_range(kind.language_id(), source, range, options)
}

/// 读取路径并格式化（不写盘）。
pub fn format_path(path: &Path, options: &FormatOptions) -> Result<FormatFileOutcome, FormatError> {
    let kind = SourceKind::from_path(path).ok_or_else(|| FormatError::unsupported_extension(path.to_path_buf()))?;
    let source = std::fs::read_to_string(path).map_err(|error| FormatError::Io { path: path.to_path_buf(), source: error })?;
    let formatted = format_source(kind, &source, options)?;
    Ok(FormatFileOutcome { path: path.to_path_buf(), changed: formatted != source, formatted })
}
