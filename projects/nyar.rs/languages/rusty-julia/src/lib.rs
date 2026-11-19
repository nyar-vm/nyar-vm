#![deny(missing_docs)]
//! Rusty Julia 语言前端

pub mod codegen;

use chomsky_uir::ConstraintAnalysis;
use nyar_types::{Id, NyarContext, TypesError, NyarFrontend, Vfs};
use data_text_core::source::SourceText;
use data_text_julia::{JuliaBuilder, JuliaLanguage, JuliaRoot};

/// Rusty Julia 前端
pub struct RustyJuliaFrontend {
    language: JuliaLanguage,
}

impl Default for RustyJuliaFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyJuliaFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { language: JuliaLanguage::default() }
    }
}

impl NyarFrontend<ConstraintAnalysis, JuliaRoot> for RustyJuliaFrontend {
    type Language = JuliaLanguage;

    fn parse(&self, source: &str) -> Result<JuliaRoot, TypesError> {
        let builder = JuliaBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = data_text_core::parser::ParseSession::<JuliaLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| TypesError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, _ast: &JuliaRoot, _ctx: &mut NyarContext<'_, V, ConstraintAnalysis>) -> Id {
        // let mut converter = crate::codegen::UirConverter::new(ctx);
        // converter.convert_root(ast)
        0
    }
}
