//! 跨语言源码格式化与 AST pretty printer 的消费门面。
//!
//! # 分层
//!
//! - 契约、注册表、`Document`/CST/`SourceMap`、editorconfig、批量与 CLI，以及 VON / AWSL 的委托实现
//!   全部由 **`language-format`** 拥有，本 crate 只做原样再导出。
//! - MSIL 文本书写器归 **`data-binary-pe`**（CLR 格式域），WAT / WIT 文本模型归 **`data-binary-wasm`**（Wasm 文本格式域）。
//! - 语言前端仍在各自 crate 里向 [`formatter::register_source_provider`] / [`printer::register_printer_provider`]
//!   注册 provider（方向是前端 → 平台）。平台自带的 WAT / WIT / MSIL printer 由
//!   [`register_platform_printers`] 显式装配，格式域 crate 不反向依赖本门面。
//!
//! 语言语义与编译不属本 crate。

#![deny(missing_docs)]

use std::any::Any;

pub use language_format::{awsl, contract, formatter, printer, von};

/// MSIL 文本书写器（模型与写出归 `data-binary-pe`）。
pub mod msil {
    pub use data_binary_pe::msil::MsilTextWriter;
}

/// WAT 文本模型与写出（归 `data-binary-wasm`）。
pub mod wat {
    pub use data_binary_wasm::text::wat::{WatDocument, WatError, format_wat_document};
}

/// WIT 文本模型与写出（归 `data-binary-wasm`）。
pub mod wit {
    pub use data_binary_wasm::text::wit::{WitError, WitInterface, WitPackage, format_wit_package};
}

struct WatPrinter;

impl contract::Printer for WatPrinter {
    fn language_id(&self) -> &str {
        "wat"
    }

    fn print(&self, document: &dyn Any, _style: contract::PrintStyle, _options: &contract::FormatOptions) -> Result<String, contract::FormatError> {
        let document = document
            .downcast_ref::<wat::WatDocument>()
            .ok_or_else(|| contract::FormatError::WrongDocument { expected: "WatDocument".into() })?;
        Ok(wat::format_wat_document(document))
    }
}

struct WitPrinter;

impl contract::Printer for WitPrinter {
    fn language_id(&self) -> &str {
        "wit"
    }

    fn print(&self, document: &dyn Any, _style: contract::PrintStyle, _options: &contract::FormatOptions) -> Result<String, contract::FormatError> {
        let package = document
            .downcast_ref::<wit::WitPackage>()
            .ok_or_else(|| contract::FormatError::WrongDocument { expected: "WitPackage".into() })?;
        Ok(wit::format_wit_package(package))
    }
}

struct MsilPrinter;

impl contract::Printer for MsilPrinter {
    fn language_id(&self) -> &str {
        "msil"
    }

    fn print(&self, document: &dyn Any, _style: contract::PrintStyle, options: &contract::FormatOptions) -> Result<String, contract::FormatError> {
        let module = document
            .downcast_ref::<data_binary_pe::msil::MsilModule>()
            .ok_or_else(|| contract::FormatError::WrongDocument { expected: "MsilModule".into() })?;
        let writer = msil::MsilTextWriter::new().with_indent_text(" ".repeat(options.indent_width));
        Ok(writer.write_module(module))
    }
}

/// 装配平台自带的 WAT / WIT / MSIL printer（幂等；先注册者优先）。
///
/// 调用点在宿主启动阶段；未调用时这三种语言 id 走 `FormatError::Unsupported`，不会静默返回空文本。
pub fn register_platform_printers() {
    printer::register_printer(&["wat"], Box::new(WatPrinter));
    printer::register_printer(&["wit"], Box::new(WitPrinter));
    printer::register_printer(&["msil", "il"], Box::new(MsilPrinter));
}
