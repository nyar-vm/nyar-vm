//! Rusty Mojo 语言前端
//!
//! 这个库提供了 Rusty Mojo 语言的词法分析、语法分析和 Gaia 翻译功能。

#![warn(missing_docs)]

use crate::codegen::MojoCodegen;
pub mod codegen;

use nyar_types::{Id, Loc, NyarContext, NyarError, NyarFrontend, Vfs};
use oak_mojo::ast::MojoStatement;
pub use oak_mojo::MojoLanguage;
use chomsky_uir::ConstraintAnalysis;

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
        Self {
            language: MojoLanguage::default(),
        }
    }
}

impl NyarFrontend<ConstraintAnalysis> for RustyMojoFrontend {
    type Language = MojoLanguage;

    fn parse(&self, source: &str) -> Result<Vec<MojoStatement>, NyarError> {
        use oak_mojo::parser::MojoParser;
        use oak_mojo::builder::MojoBuilder;
        use oak_core::{
            parser::{Parser, session::ParseSession},
            source::SourceText,
        };
        
        let parser = MojoParser::new();
        let mut session = ParseSession::<MojoLanguage>::default();
        let output = parser.parse(source, &[], &mut session);
        
        let green = output.result.map_err(|e| NyarError::Compile(format!("{:?}", e)))?;
        
        let source_text = SourceText::new(source.to_string());
        let builder = MojoBuilder::new(&source_text);
        builder.build_root(green).map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &Vec<MojoStatement>, ctx: &mut NyarContext<V, ConstraintAnalysis>) -> Id {
        let mut codegen = MojoCodegen::new(ctx);
        codegen.lower_statements(ast)
    }
}
