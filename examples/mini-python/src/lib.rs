//! Mini Python 语言前端
//!
//! 这个库提供了 Mini Python 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;
pub mod pyc_codegen;

use oak_python::{ast::Program, lexer::PythonLexer, parser::PythonParser};
use codegen::GaiaTranslator;
use gaia_assembler::program::GaiaModule;
use gaia_types::GaiaError;
use pyc_codegen::{Marshal, PycTranslator};
use oak_core::ParseError;

/// Mini Python 前端
pub struct MiniPythonFrontend {
    translator: GaiaTranslator,
}

impl MiniPythonFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { translator: GaiaTranslator::new() }
    }

    /// 解析 Python 源代码为 AST
    pub fn parse(&mut self, source: &str) -> Result<Program, ParseError> {
        let mut lexer = PythonLexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = PythonParser::new(tokens);
        parser.parse()
    }

    /// 将 Python 源代码编译为 Gaia 程序
    pub fn compile_to_gaia(&mut self, source: &str) -> Result<GaiaModule, GaiaError> {
        // 解析为 AST
        let ast = self
            .parse(source)
            .map_err(|e| GaiaError::syntax_error(&format!("Parse error: {:?}", e), gaia_types::SourceLocation::default()))?;

        // 翻译为 Gaia 程序
        self.translator.generate(&ast)
    }

    /// 将 Python 源代码编译为 .pyc 字节流
    pub fn compile_to_pyc(&mut self, source: &str, filename: &str) -> Result<Vec<u8>, String> {
        let ast = self.parse(source).map_err(|e| format!("Parse error: {:?}", e))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mini_python_frontend() {
        let code = r#"
def hello(name):
    print("Hello")
    return True

x = 42
y = [1, 2, 3]
"#;

        let mut frontend = MiniPythonFrontend::new();
        let result = frontend.parse(code);
        assert!(result.is_ok());

        let program = result.unwrap();
        assert_eq!(program.statements.len(), 3); // function, assignment x, assignment y
    }

    #[test]
    fn test_tokenize_only() {
        let code = "def hello(): pass";
        let mut frontend = MiniPythonFrontend::new();
        let tokens = frontend.tokenize(code);
        assert!(tokens.is_ok());
    }
}
