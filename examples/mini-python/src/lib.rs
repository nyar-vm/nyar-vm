//! Mini Python 语言前端
//!
//! 这个库提供了 Mini Python 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;
pub mod pyc_codegen;

use oak_python::{ast::Program, lexer::PythonLexer, parser::PythonParser, PythonFrontend};
use codegen::GaiaTranslator;
use gaia_assembler::program::GaiaModule;
use gaia_types::GaiaError;
use pyc_codegen::{Marshal, PycTranslator};
use oak_core::ParseError;
use chomsky_uir::{EGraph, Id};

/// Mini Python 前端
pub struct MiniPythonFrontend {
    translator: GaiaTranslator,
}

impl MiniPythonFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { translator: GaiaTranslator::new() }
    }

    /// 解析 Python 源代码为 UIR
    pub fn parse(&mut self, source: &str) -> Result<(EGraph, Id), String> {
        let frontend = PythonFrontend::new(source);
        frontend.parse()
    }

    /// 将 Python 源代码编译为 Gaia 程序
    pub fn compile_to_gaia(&mut self, source: &str) -> Result<GaiaModule, GaiaError> {
        // 解析为 UIR
        let (egraph, root) = self.parse(source).map_err(|e| GaiaError::syntax_error(&format!("Parse error: {:?}", e), gaia_types::SourceLocation::default()))?;

        // 翻译为 Gaia 程序
        self.translator.generate(&egraph, root)
    }

    /// 将 Python 源代码编译为 .pyc 字节流
    pub fn compile_to_pyc(&mut self, source: &str, filename: &str) -> Result<Vec<u8>, String> {
        // PycTranslator currently uses AST Program.
        // We need to keep AST parsing for PycTranslator unless we refactor it too.
        // oak-python::PythonFrontend uses PythonParser which returns Program.
        // My update to oak-python::lib.rs makes parse() return (EGraph, Id), but it calls converter.
        // I can still access Parser directly if needed.
        
        let mut lexer = PythonLexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;
        let mut parser = PythonParser::new(tokens);
        let ast = parser.parse().map_err(|e| format!("{:?}", e))?;

        let mut translator = PycTranslator::new(filename, "<module>");
        let code_obj = translator.translate(&ast);

        let mut marshal = Marshal::new();
        marshal.write_code_object(&code_obj);
        let code_data = marshal.finish();
        println!("Generated code_data: {:?}", code_data);

        // 构造 .pyc 文件头 (Python 3.10)
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
    pub fn tokenize(&mut self, source: &str) -> Result<Vec<gaia_types::reader::Token<oak_python::lexer::PythonTokenType>>, ParseError> {
        let mut lexer = PythonLexer::new(source);
        let token_stream = lexer.tokenize()?;
        // 从 TokenStream 中提取 tokens
        Ok(token_stream.tokens.into_inner())
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
