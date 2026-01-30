//! Mini Rust 语言实现
#![feature(new_range_api)]

pub mod ast;
pub mod codegen;
pub mod converter;

use nyar_error::NyarError;
use nyar_frontend::NyarFrontend;
use nyar_types::IKunTree;
use oak_core::source::SourceText;
use oak_rust::{RustBuilder, RustLanguage, RustRoot};

/// Mini Rust 前端实现
#[derive(Default)]
pub struct MiniRustFrontend {
    language: RustLanguage,
}

impl MiniRustFrontend {
    /// 创建一个新的 Mini Rust 前端
    pub fn new() -> Self {
        Self {
            language: RustLanguage::default(),
        }
    }
}

impl NyarFrontend for MiniRustFrontend {
    type Language = RustLanguage;

    fn parse(&self, source: &str) -> Result<RustRoot, NyarError> {
        let builder = RustBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<RustLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &RustRoot) -> Result<IKunTree, NyarError> {
        let translator = codegen::GaiaTranslator::new();
        translator.translate_to_tree(_ast)
    }
}
