//! Rusty Dart 语言前端
//!
//! 这个库提供了 Rusty Dart 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{NyarContext, NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_dart::{DartBuilder, DartLanguage, DartRoot};
use oak_vfs::Vfs;
use chomsky_uir::Id;

/// Rusty Dart 前端
#[derive(Default)]
pub struct RustyDartFrontend;

impl RustyDartFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self
    }
}

impl NyarFrontend for RustyDartFrontend {
    type Language = DartLanguage;

    fn parse(&self, source: &str) -> Result<DartRoot, NyarError> {
        let language = DartLanguage::default();
        let builder = DartBuilder::new(&language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<DartLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &DartRoot, ctx: &mut NyarContext<V>) -> Id {
        let mut converter = codegen::UirConverter::new(ctx);
        converter.convert_root(ast)
    }
}
