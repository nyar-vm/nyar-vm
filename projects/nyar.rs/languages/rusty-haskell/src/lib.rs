//! Rusty Haskell 语言前端
//!
//! 这个库提供了 Rusty Haskell 语言的词法分析、语法分析和 Nyar 翻译功能。

#![deny(missing_docs)]

pub mod codegen;

use nyar_types::{Id, NyarContext, TypesError, NyarFrontend, Vfs};
use data_text_core::{source::SourceText, Builder};
use data_text_haskell::HaskellLanguage;

/// Rusty Haskell 前端
pub struct RustyHaskellFrontend {
    language: HaskellLanguage,
}

impl Default for RustyHaskellFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyHaskellFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { language: HaskellLanguage::new() }
    }
}

impl NyarFrontend<(), data_text_haskell::ast::HaskellRoot> for RustyHaskellFrontend {
    type Language = HaskellLanguage;

    fn parse(&self, source: &str) -> Result<data_text_haskell::ast::HaskellRoot, TypesError> {
        let source_text = SourceText::new(source);
        let mut session = data_text_core::parser::ParseSession::<HaskellLanguage>::default();
        let builder = data_text_haskell::builder::HaskellBuilder::new(&self.language);
        let output = builder.build(&source_text, &[], &mut session);

        output.result.map_err(|e| TypesError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, _ast: &data_text_haskell::ast::HaskellRoot, _ctx: &mut NyarContext<'_, V>) -> Id {
        let _translator = crate::codegen::NyarTranslator::new();
        // let mut translator_ctx = crate::codegen::TranslatorContext::new_with_builder(ctx.builder());
        // translator.translate_root(ast, &mut translator_ctx).unwrap_or_else(|_| ctx.egraph.add(chomsky_uir::IKun::Seq(vec![])))
        0
    }
}
