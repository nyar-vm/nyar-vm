//! Mini Rust 语言实现
#![feature(new_range_api)]

pub mod ast;
// pub mod codegen;
pub mod converter;

use chomsky_extract::Backend;
use chomsky_uir::ConstraintAnalysis;
use chomsky_uir::IntentBuilder;
use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::source::SourceText;
use oak_core::Builder;
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
        output
            .result
            .map_err(|_| NyarError::Parse())
    }

    fn lower(&self, _ast: &RustRoot) -> Result<IKunTree, NyarError> {
        let mut aot = nyar_aot::NyarAot::<ConstraintAnalysis>::new();
        let mut builder = chomsky_uir::IntentBuilder::new(&mut aot.optimizer.egraph);
        let id = converter::convert_root(_ast, &mut builder);

        aot.saturate();

        let backend = nyar_vm::bytecode::compiler::NyarBackend::new();
        let tree = aot.extract(id, backend.get_model());

        Ok(tree)
    }
}
