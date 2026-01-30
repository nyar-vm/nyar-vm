//! Mini Lua 语言前端
//!
//! 这个库提供了 Mini Lua 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use oak_lua::{ast::LuaRoot, LuaLanguage, LuaBuilder};
use nyar_types::{NyarFrontend, NyarError, IKunTree};
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

    fn lower(&self, _ast: &LuaRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 LuaRoot 到 IKunTree 的转换
        let mut tree = IKunTree::default();
        tree.name = "mini-lua-program".to_string();
        Ok(tree)
    }
}
