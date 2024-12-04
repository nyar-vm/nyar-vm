//! Valkyrie 前端向 `nyar-format` 平台注册格式化 provider（依赖方向：前端 → 平台）。
//!
//! 平台层不反向依赖本 crate；`legion` / `asgard` CLI 与集成测试在格式化入口调用
//! [`register_format_providers`]。formatter 直接委托 `oak_valkyrie::formatter`（CST token-gap），
//! printer 直接委托 `oak_valkyrie::printer` 并以 [`ValkyrieRoot`] 作为下转类型。

use std::any::Any;

use nyar_analyzer::format::{
    FormatError, FormatOptions, FormattedOutput, PrintStyle, Printer, PrinterProvider, SourceFormatter, SourceFormatterProvider,
};
use oak_valkyrie::ast::ValkyrieRoot;

/// Valkyrie 源码 formatter 的 provider（`.v` / `.vx`）。
pub struct ValkyrieSourceFormatterProvider;

impl SourceFormatterProvider for ValkyrieSourceFormatterProvider {
    fn language_ids(&self) -> &[&str] {
        &["v", "vx"]
    }

    fn formatter(&self) -> Box<dyn SourceFormatter> {
        Box::new(ValkyrieSourceFormatter)
    }
}

struct ValkyrieSourceFormatter;

impl SourceFormatter for ValkyrieSourceFormatter {
    fn language_id(&self) -> &str {
        "v"
    }

    fn format_document(&self, source: &str, options: &FormatOptions) -> Result<FormattedOutput, FormatError> {
        format_valkyrie_source(source, options)
    }
}

/// Valkyrie AST printer 的 provider（`v` / `vx` / `valkyrie`）。
pub struct ValkyriePrinterProvider;

impl PrinterProvider for ValkyriePrinterProvider {
    fn language_ids(&self) -> &[&str] {
        &["v", "vx", "valkyrie"]
    }

    fn printer(&self) -> Box<dyn Printer> {
        Box::new(ValkyriePrinter)
    }
}

struct ValkyriePrinter;

impl Printer for ValkyriePrinter {
    fn language_id(&self) -> &str {
        "v"
    }

    fn print(&self, document: &dyn Any, style: PrintStyle, options: &FormatOptions) -> Result<String, FormatError> {
        let root = document.downcast_ref::<ValkyrieRoot>().ok_or_else(|| FormatError::WrongDocument { expected: "ValkyrieRoot".into() })?;
        print_valkyrie_root(root, style, options)
    }
}

/// Valkyrie / VX 源码正规格式化（CST token-gap 路径，直接委托 `oak_valkyrie::formatter`）。
pub(crate) fn format_valkyrie_source(source: &str, options: &FormatOptions) -> Result<FormattedOutput, FormatError> {
    let oak_options =
        oak_valkyrie::formatter::FormatOptions { indent_width: options.indent_width.min(255) as u8, line_width: options.max_width };
    let text = oak_valkyrie::formatter::format_source(source, &oak_options)
        .map_err(|error| FormatError::Parse { path: None, message: error.to_string() })?;
    let mut out = text;
    if options.ensure_trailing_newline && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(FormattedOutput::text_only(out))
}

/// 将 `oak_valkyrie::ast::ValkyrieRoot` 写出为文本（AST print；非 CST formatter）。
pub fn print_valkyrie_root(root: &ValkyrieRoot, style: PrintStyle, options: &FormatOptions) -> Result<String, FormatError> {
    oak_valkyrie::printer::print_root(root, &map_print_options(style, options)).map_err(map_print_error)
}

/// Valkyrie 源码 AST print（parse → `Document` → text；非 CST formatter）。
pub fn print_valkyrie_source(source: &str, style: PrintStyle, options: &FormatOptions) -> Result<String, FormatError> {
    oak_valkyrie::printer::print_source(source, &map_print_options(style, options)).map_err(map_print_error)
}

fn map_print_style(style: PrintStyle) -> oak_valkyrie::printer::PrintStyle {
    match style {
        PrintStyle::Compact => oak_valkyrie::printer::PrintStyle::Compact,
        PrintStyle::Indented => oak_valkyrie::printer::PrintStyle::Indented,
    }
}

fn map_print_options(style: PrintStyle, options: &FormatOptions) -> oak_valkyrie::printer::PrintOptions {
    oak_valkyrie::printer::PrintOptions {
        style: map_print_style(style),
        indent_width: options.indent_width,
        max_width: options.max_width,
    }
}

fn map_print_error(error: oak_valkyrie::printer::PrintError) -> FormatError {
    match error {
        oak_valkyrie::printer::PrintError::Parse(message) => FormatError::Parse { path: None, message },
        oak_valkyrie::printer::PrintError::Unsupported { context } => FormatError::Parse { path: None, message: context },
    }
}

/// 向 `nyar-format` 注册 Valkyrie formatter / printer provider（幂等，可在入口重复调用）。
pub fn register_format_providers() {
    nyar_format::formatter::register_source_provider(&ValkyrieSourceFormatterProvider);
    nyar_format::printer::register_printer_provider(&ValkyriePrinterProvider);
}
