#![feature(new_range_api)]
//! Clojure 语言前端
//!
//! 提供 Clojure 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;
pub mod errors;
pub mod row_type;
pub mod tagless;
pub mod visitor;
pub mod runtime;

pub use crate::runtime::RustyClojureRuntime;

use nyar_aot::{NyarContext, NyarFrontend};
use nyar_types::NyarError;
use oak_core::source::SourceText;
use oak_core::parser::{Parser, session::ParseSession};
use oak_clojure::{ClojureLanguage, ClojureParser};
use oak_vfs::Vfs;
use chomsky_uir::{Id, ConstraintAnalysis};

/// Rusty Clojure 前端
pub struct RustyClojureFrontend {
    language: ClojureLanguage,
}

impl Default for RustyClojureFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyClojureFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let language = ClojureLanguage::default();
        Self { language }
    }
}

impl NyarFrontend<ConstraintAnalysis> for RustyClojureFrontend {
    type Language = ClojureLanguage;

    fn parse(&self, source: &str) -> Result<(), NyarError> {
        let parser = ClojureParser::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = ParseSession::<ClojureLanguage>::default();
        let output = parser.parse(&source_text, &[], &mut session);
        if output.result.is_ok() {
            println!("Parsed Clojure source");
        }
        output
            .result
            .map(|_| ())
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &(), ctx: &mut NyarContext<V, ConstraintAnalysis>) -> Id {
        let translator = codegen::NyarTranslator::new();
        translator.translate_to_id(ast, &mut ctx.builder()).unwrap_or_else(|_| ctx.egraph.add(chomsky_uir::IKun::Seq(vec![])))
    }
}
