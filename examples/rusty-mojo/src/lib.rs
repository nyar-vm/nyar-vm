//! Rusty Mojo 语言前端
//!
//! 这个库提供了 Rusty Mojo 语言的词法分析、语法分析和 Gaia 翻译功能。

#![deny(missing_docs)]

use crate::codegen::MojoCodegen;
pub mod codegen;

use chomsky_uir::ConstraintAnalysis;
use nyar_types::{Id, Loc, NyarContext, NyarError, NyarFrontend, Vfs};
use oak_mojo::ast::MojoStatement;
pub use oak_mojo::MojoLanguage;

/// Rusty Mojo 前端
pub struct RustyMojoFrontend {
    language: MojoLanguage,
}

impl Default for RustyMojoFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyMojoFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { language: MojoLanguage::default() }
    }
}

impl NyarFrontend<ConstraintAnalysis, Vec<MojoStatement>> for RustyMojoFrontend {
    type Language = MojoLanguage;

    fn parse(&self, source: &str) -> Result<Vec<MojoStatement>, NyarError> {
        use oak_core::{parser::Parser, source::SourceText};
        use oak_mojo::{builder::MojoBuilder, parser::MojoParser};

        let parser = MojoParser::new();
        let mut session = oak_core::parser::ParseSession::<MojoLanguage>::default();
        let output = parser.parse(source, &[], &mut session);

        let green = output.result.map_err(|e| NyarError::Compile(format!("{:?}", e)))?;

        let source_text = SourceText::new(source.to_string());
        let builder = MojoBuilder::new(&source_text);
        builder.build_root(green).map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &Vec<MojoStatement>, ctx: &mut NyarContext<'_, V, ConstraintAnalysis>) -> Id {
        let mut _codegen = MojoCodegen::new(ctx);
        // codegen.lower_statements(ast)
        0
    }
}
