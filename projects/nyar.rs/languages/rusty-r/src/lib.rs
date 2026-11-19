#![deny(missing_docs)]
#![feature(new_range_api)]
//! Rusty R 语言前端

pub mod codegen;

use chomsky_uir::ConstraintAnalysis;
use nyar_types::{Id, NyarContext, TypesError, NyarFrontend, Vfs};
use data_text_core::source::SourceText;
use data_text_r::{ast::RRoot, RBuilder, RLanguage};

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
        Self { language: RLanguage::new() }
    }
}

impl NyarFrontend<ConstraintAnalysis, RRoot> for RustyRFrontend {
    type Language = RLanguage;

    fn parse(&self, source: &str) -> Result<RRoot, TypesError> {
        let builder = RBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = data_text_core::parser::ParseSession::<RLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| TypesError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, _ast: &RRoot, _ctx: &mut NyarContext<'_, V, ConstraintAnalysis>) -> Id {
        // let translator = crate::codegen::NyarTranslator::new();
        // let mut translator_ctx = crate::codegen::TranslatorContext::new_with_builder(ctx.builder());
        // translator
        //     .translate_root(ast, &mut translator_ctx)
        //     .unwrap_or_else(|_| ctx.egraph.add(chomsky_uir::IKun::Seq(vec![])))
        0
    }
}
