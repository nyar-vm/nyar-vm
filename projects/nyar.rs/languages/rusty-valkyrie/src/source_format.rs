//! Valkyrie (`.v` / `.vx`) source formatter（Oak token-gap）。
//!
//! 实现位于 [`crate::format_provider`]（平台 provider 的直接路径），本入口只保留给 LSP 等
//! 前端内部调用，确保仓库内只有一套 V formatter 实现。

use nyar_analyzer::format::{FormatError, FormatOptions, FormattedOutput};

/// Valkyrie / VX 源码正规格式化（CST token-gap 路径）。
pub fn format_valkyrie(source: &str, options: &FormatOptions, _vx: bool) -> Result<FormattedOutput, FormatError> {
    crate::format_provider::format_valkyrie_source(source, options)
}
