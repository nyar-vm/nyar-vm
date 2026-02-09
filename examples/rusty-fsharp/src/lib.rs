//! Rusty FSharp 语言前端
//!
//! 这个库提供了 Rusty FSharp 语言的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_fsharp::{FSharpLanguage};

/// Rusty FSharp 前端
pub struct RustyFSharpFrontend {
    language: FSharpLanguage,
}

impl Default for RustyFSharpFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyFSharpFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: FSharpLanguage::new(),
        }
    }
}

impl NyarFrontend for RustyFSharpFrontend {
    type Language = FSharpLanguage;

    fn parse(&self, source: &str) -> Result<oak_fsharp::ast::FSharpRoot, NyarError> {
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<FSharpLanguage>::default();
        let builder = oak_fsharp::builder::FSharpBuilder::new(&self.language);
        let output = builder.build(&source_text, &[], &mut session);

        output.result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: nyar_types::Vfs>(
        &self,
        ast: &oak_fsharp::ast::FSharpRoot,
        ctx: &mut nyar_types::NyarContext<'_, V>,
    ) -> chomsky_uir::Id {
        let translator = crate::codegen::NyarTranslator::new();
        let mut translator_ctx = crate::codegen::TranslatorContext::new_with_builder(ctx.builder());
        translator.translate_root(ast, &mut translator_ctx).unwrap_or_else(|_| ctx.egraph.add(chomsky_uir::IKun::Seq(vec![])))
    }
}
