//! Rusty R 语言前端
//!
//! 这个库提供了 Rusty R 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_r::{ast::RRoot, RBuilder, RLanguage};

/// Rusty R 前端
pub struct RustyRFrontend {
    language: RLanguage,
}

impl Default for RustyRFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyRFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: RLanguage::new(),
        }
    }
}

impl NyarFrontend for RustyRFrontend {
    type Language = RLanguage;

    fn parse(&self, source: &str) -> Result<RRoot, NyarError> {
        let builder = RBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<RLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: nyar_types::Vfs>(
        &self,
        ast: &RRoot,
        ctx: &mut nyar_types::NyarContext<'_, V>,
    ) -> chomsky_uir::egraph::Id {
        let translator = crate::codegen::NyarTranslator::new();
        let mut translator_ctx = crate::codegen::TranslatorContext::new_with_builder(ctx.builder());
        translator
            .translate_root(ast, &mut translator_ctx)
            .unwrap_or_else(|_| ctx.egraph.add(chomsky_uir::IKun::Seq(vec![])))
    }
}
