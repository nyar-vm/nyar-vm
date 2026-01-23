//! Mini Python 语言前端
//!
//! 这个库提供了 Mini Python 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;
pub mod pyc_codegen;

use oak_python::{ast::Program, PythonLanguage, PythonFrontend};
use codegen::GaiaTranslator;
use gaia_assembler::program::GaiaModule;
use gaia_types::GaiaError;
use pyc_codegen::{Marshal, PycTranslator};
use oak_core::{Lexer, Parser, ParseError, lexer::LexerCache, source::SourceText};
use chomsky_uir::{EGraph, Id, IKunTree, DEFAULT_COST_MODEL};
use chomsky_full::optimizer::UniversalOptimizer;
// use chomsky_cost::DEFAULT_COST_MODEL;

/// Mini Python 前端
pub struct MiniPythonFrontend {
    translator: GaiaTranslator,
    optimizer: UniversalOptimizer<()>,
}

impl MiniPythonFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { 
            translator: GaiaTranslator::new(),
            optimizer: UniversalOptimizer::new(),
        }
    }

    /// 解析 Python 源代码为 EGraph
    pub fn parse_to_egraph(&mut self, source: &str) -> Result<Id, String> {
        let source_text = SourceText::new(source);
        let frontend = PythonFrontend::new(&source_text);
        let mut builder = chomsky_uir::builder::IntentBuilder::new(&mut self.optimizer.egraph);
        frontend.parse(&mut builder).map_err(|e| format!("{:?}", e))
    }

    /// 解析 Python 源代码为 AST (用于 --ast 调试)
    pub fn parse_to_ast(&mut self, source: &str) -> Result<Program, String> {
        let source_text = SourceText::new(source);
        let frontend = PythonFrontend::new(&source_text);
        frontend.parse_to_ast().map_err(|e| format!("{:?}", e))
    }

    /// 将 Python 源代码编译为 Gaia 程序
    pub fn compile_to_gaia(&mut self, source: &str) -> Result<GaiaModule, GaiaError> {
        // 1. 解析为 EGraph
        let root = self.parse_to_egraph(source).map_err(|e| GaiaError::syntax_error(&format!("Parse error: {}", e), gaia_types::SourceLocation::default()))?;

        // 2. 优化：等价饱和
        self.optimizer.saturate();

        // 3. 提取：基于成本模型提取最优意图树
        let optimized_tree = self.optimizer.extract(root, DEFAULT_COST_MODEL);

        // 4. 后端生成：将优化后的意图树翻译为 Gaia 程序
        self.translator.generate_from_tree(&optimized_tree)
    }

    /// 将 Python 源代码编译为 .pyc 字节流
    pub fn compile_to_pyc(&mut self, source: &str, filename: &str) -> Result<Vec<u8>, String> {
        // 1. 解析为 EGraph
        let root = self.parse_to_egraph(source).map_err(|e| format!("Parse error: {}", e))?;

        // 2. 优化：等价饱和
        self.optimizer.saturate();

        // 3. 提取：基于成本模型提取最优意图树
        let optimized_tree = self.optimizer.extract(root, DEFAULT_COST_MODEL);

        // 4. 后端生成：将优化后的意图树翻译为 Python 字节码
        let mut translator = PycTranslator::new(filename, "<module>");
        let code_obj = translator.translate_from_tree(&optimized_tree);

        let mut marshal = Marshal::new();
        marshal.write_code_object(&code_obj);
        let code_data = marshal.finish();

        // 构造 .pyc 文件头 (Python 3.12 magic number)
        let mut pyc_data = Vec::new();
        // Magic number for Python 3.12: 0xcb0d0d0a
        pyc_data.extend_from_slice(&[0xcb, 0x0d, 0x0d, 0x0a]);
        // Bit field (0)
        pyc_data.extend_from_slice(&[0, 0, 0, 0]);
        // Timestamp (current time or 0)
        pyc_data.extend_from_slice(&[0, 0, 0, 0]);
        // File size
        pyc_data.extend_from_slice(&(source.len() as u32).to_le_bytes());

        pyc_data.extend_from_slice(&code_data);
        Ok(pyc_data)
    }

    /// 仅进行词法分析
    pub fn tokenize(&mut self, source: &str) -> Result<Vec<oak_core::lexer::Token<oak_python::kind::PythonSyntaxKind>>, ParseError> {
        let config = PythonLanguage;
        let lexer = oak_python::lexer::PythonLexer::new(&config);
        let mut cache = oak_core::lexer::ParseSession::<PythonLanguage>::default();
        let source_text = SourceText::new(source);
        let output = lexer.lex(&source_text, &[], &mut cache);
        
        match output.result {
            Ok(tokens) => Ok(tokens.to_vec()),
            Err(e) => Err(e),
        }
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

impl Default for MiniPythonFrontend {
    fn default() -> Self {
        Self::new()
    }
}
