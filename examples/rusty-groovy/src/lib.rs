#![feature(new_range_api)]
//! Groovy 语言前端
//!
//! 提供 Groovy 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;
pub mod errors;
pub mod row_type;
pub mod tagless;
pub mod visitor;

use nyar_types::{NyarContext, NyarError, NyarFrontend, Id, Vfs};
use oak_core::source::SourceText;
use oak_groovy::language::GroovyLanguage;
use oak_groovy::parser::GroovyParser;
use oak_core::parser::Parser;
use chomsky_uir::ConstraintAnalysis;

/// Rusty Groovy 前端
pub struct RustyGroovyFrontend {
    language: GroovyLanguage,
}

impl Default for RustyGroovyFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyGroovyFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let language = GroovyLanguage::default();
        Self { language }
    }
}

impl NyarFrontend<ConstraintAnalysis> for RustyGroovyFrontend {
    type Language = GroovyLanguage;

    fn parse(&self, source: &str) -> Result<(), NyarError> {
        let parser = GroovyParser::new(&self.language);
        let source_text = SourceText::new(source);
        let mut cache = oak_core::parser::SimpleParseCache::default();
        let output = parser.parse(&source_text, &[], &mut cache);
        if output.result.is_ok() {
            println!("Parsed Groovy source");
        }
        output
            .result
            .map(|_| ())
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &(), ctx: &mut NyarContext<V, ConstraintAnalysis>) -> Id {
        let mut translator = codegen::NyarTranslator::new();
        translator.translate_to_id(ast, &mut ctx.builder()).unwrap_or_else(|_| ctx.egraph.add(chomsky_uir::IKun::Seq(vec![])))
    }
}
