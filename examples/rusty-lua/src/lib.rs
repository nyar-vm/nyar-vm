//! Mini Lua 语言前端
//!
//! 这个库提供了 Mini Lua 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use oak_lua::{ast::LuaRoot, LuaLanguage, LuaBuilder, LuaParser, lexer::LuaLexer};
use codegen::GaiaTranslator;
use gaia_assembler::program::GaiaModule;
use gaia_types::GaiaError;
use oak_core::{Lexer, OakError, ParseSession, source::SourceText, Parser, Builder, BuilderCache, tree::RedNode};
use chomsky_uir::{Id, IntentBuilder, IKun};
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
        let language = LuaLanguage;
        let lexer = LuaLexer::new(&language);
        let mut session = ParseSession::<LuaLanguage>::default();
        
        let source_text = SourceText::new(source);
        
        lexer.lex(&source_text, &[], &mut session);
        
        let parser = LuaParser::new(&language);
        let parse_output = parser.parse(&source_text, &[], &mut session);
        
        let green_node = parse_output.result.map_err(|e| format!("Parse error: {:?}", e))?;
        let red_node = RedNode::new(green_node.clone(), 0);
        
        let mut builder = IntentBuilder::new(&mut self.optimizer.egraph);
        
        // TODO: 实现更完整的 Lua 到 UIR 的转换
        let root_id = self.convert_red_to_uir(&mut builder, red_node, source);
        
        Ok(root_id)
    }

    fn convert_red_to_uir(&self, builder: &mut IntentBuilder<()>, node: RedNode<LuaLanguage>, _source: &str) -> Id {
        let loc = chomsky_source::Loc::default();
        // 这是一个极简的转换实现，只处理根节点
        builder.extension("lua_module", vec![builder.string("mini_lua_program", loc)], loc)
    }

    /// 解析 Lua 源代码为 AST (用于 --ast 调试)
    pub fn parse_to_ast(&mut self, source: &str) -> Result<LuaRoot, String> {
        let language = LuaLanguage;
        let builder = LuaBuilder::new(&language);
        let source_text = SourceText::new(source);
        let mut session = ParseSession::<LuaLanguage>::default();
        
        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| format!("{:?}", e))
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
        let language = LuaLanguage;
        let lexer = LuaLexer::new(&language);
        let source_text = SourceText::new(source);
        let mut session = ParseSession::<LuaLanguage>::default();
        let output = lexer.lex(&source_text, &[], &mut session);
        output.result
    }
}
