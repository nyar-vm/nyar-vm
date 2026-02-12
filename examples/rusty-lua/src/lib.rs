//! Rusty Lua 语言前端
//!
//! 这个库提供了 Rusty Lua 语言的词法分析、语法分析和 Gaia 翻译功能。

#![warn(missing_docs)]

pub mod codegen;
pub mod runtime;

pub use crate::runtime::RustyLuaRuntime;

use nyar_types::{Id, Loc, NyarContext, NyarError, NyarFrontend, Vfs};
use oak_core::{source::SourceText, Builder};
use oak_lua::{ast::LuaRoot, LuaBuilder, LuaLanguage};
use chomsky_uir::{Analysis, IKun, egraph::HasDebugInfo};

/// Rusty Lua 前端
pub struct RustyLuaFrontend {
    language: LuaLanguage,
}

impl Default for RustyLuaFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyLuaFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: LuaLanguage {},
        }
    }
}

impl<A: Analysis<IKun> + 'static> NyarFrontend<A> for RustyLuaFrontend 
where A::Data: HasDebugInfo
{
    type Language = LuaLanguage;

    fn parse(&self, source: &str) -> Result<LuaRoot, NyarError> {
        let builder = LuaBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::session::ParseSession::<LuaLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &LuaRoot, ctx: &mut NyarContext<V, A>) -> Id {
        let translator = codegen::GaiaTranslator::new();
        translator.lower_unified(ast, ctx)
    }
}
