//! Mini Lua 语言前端
//!
//! 这个库提供了 Mini Lua 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use oak_lua::{ast::LuaRoot, LuaLanguage, LuaBuilder};
use nyar_vm::{NyarFrontend, NyarError};
use chomsky_uir::IKunTree;
use oak_core::{source::SourceText, Builder};

/// Mini Lua 前端
pub struct MiniLuaFrontend {
    language: LuaLanguage,
}

impl Default for MiniLuaFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniLuaFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { 
            language: LuaLanguage,
        }
    }
}

impl NyarFrontend for MiniLuaFrontend {
    type Language = LuaLanguage;

    fn parse(&self, source: &str) -> Result<LuaRoot, NyarError> {
        let builder = LuaBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<LuaLanguage>::default();
        
        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, ast: &LuaRoot) -> Result<IKunTree, NyarError> {
        let translator = codegen::GaiaTranslator::new();
        translator.translate_to_tree(ast)
    }
}
