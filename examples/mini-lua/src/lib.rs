//! Mini Lua 语言前端
//!
//! 这个库提供了 Mini Lua 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use oak_lua::{ast::LuaRoot, LuaLanguage, LuaParser};
use codegen::GaiaTranslator;
use gaia_assembler::program::GaiaModule;
use gaia_types::GaiaError;
use oak_core::{Lexer, OakError, ParseSession, source::SourceText};
use chomsky_uir::Id;
use chomsky_cost::DefaultCostModel;
use chomsky_full::optimizer::UniversalOptimizer;

/// Mini Lua 前端
pub struct MiniLuaFrontend {
    translator: GaiaTranslator,
    optimizer: UniversalOptimizer<()>,
}

impl MiniLuaFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { 
            translator: GaiaTranslator::new(),
            optimizer: UniversalOptimizer::new(),
        }
    }

    /// 解析 Lua 源代码为 EGraph
    pub fn parse_to_egraph(&mut self, source: &str) -> Result<Id, String> {
        let source_text = SourceText::new(source);
        let mut builder = chomsky_uir::builder::IntentBuilder::new(&mut self.optimizer.egraph);
        let parser = LuaParser::new(&source_text);
        parser.parse(&mut builder).map_err(|e| format!("{:?}", e))
    }

    /// 解析 Lua 源代码为 AST (用于 --ast 调试)
    pub fn parse_to_ast(&mut self, source: &str) -> Result<LuaRoot, String> {
        let source_text = SourceText::new(source);
        let parser = LuaParser::new(&source_text);
        parser.parse_to_ast().map_err(|e| format!("{:?}", e))
    }

    /// 将 Lua 源代码编译为 Gaia 程序
    pub fn compile_to_gaia(&mut self, source: &str) -> Result<GaiaModule, GaiaError> {
        // 1. 解析为 EGraph
        let root = self.parse_to_egraph(source).map_err(|e| GaiaError::syntax_error(&format!("Parse error: {}", e), gaia_types::SourceLocation::default()))?;

        // 2. 优化：等价饱和
        self.optimizer.saturate();

        // 3. 提取：基于成本模型提取最优意图树
        let optimized_tree = self.optimizer.extract(root, DefaultCostModel::default());

        // 4. 后端生成：将优化后的意图树翻译为 Gaia 程序
        self.translator.generate_from_tree(&optimized_tree)
    }

    /// 仅进行词法分析
    pub fn tokenize(&mut self, source: &str) -> Result<Vec<oak_core::lexer::Token<oak_lua::kind::LuaSyntaxKind>>, OakError> {
        let config = LuaLanguage;
        let lexer = oak_lua::lexer::LuaLexer::new(&config);
        lexer.tokenize(source)
    }

    /// 获取翻译器的可变引用
    pub fn translator_mut(&mut self) -> &mut GaiaTranslator {
        &mut self.translator
    }

    /// 获取翻译器的不可变引用
    pub fn translator(&self) -> &GaiaTranslator {
        &self.translator
    }
}

impl Default for MiniLuaFrontend {
    fn default() -> Self {
        Self::new()
    }
}
