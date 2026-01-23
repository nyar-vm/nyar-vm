//! Mini C 语言前端
//!
//! 这个库提供了 Mini C 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod ast;
pub mod codegen;
pub mod config;
pub mod lexer;
pub mod parser;

use ast::Program;
use codegen::GaiaTranslator;
use config::ReadConfig;
use gaia_assembler::{assembler::GaiaAssembler, program::GaiaModule};
use gaia_types::{
    helpers::CompilationTarget,
    GaiaError,
};
use lexer::CLexer;
use parser::CParser;

/// Mini C 前端
pub struct MiniCFrontend {
    config: ReadConfig,
    translator: GaiaTranslator,
}

impl MiniCFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { config: ReadConfig::new(), translator: GaiaTranslator::new() }
    }

    /// 解析 C 源代码为 AST
    pub fn parse(&mut self, source: &str) -> Result<Program, GaiaError> {
        let lexer = CLexer::new(&self.config);
        let token_stream = lexer.tokenize(source).result?;
        let mut parser = CParser::new(token_stream);
        parser.parse()
    }

    /// 将 C 源代码编译为 Gaia 程序
    pub fn compile_to_gaia(&mut self, source: &str) -> Result<GaiaModule, GaiaError> {
        // 解析为 AST
        let ast = self.parse(source)?;

        // 翻译为 Gaia 程序
        self.translator.generate(&ast)
    }

    /// 编译 C 源代码到二进制文件
    pub fn compile_to_binary(&mut self, source: &str, target: CompilationTarget) -> Result<Vec<u8>, GaiaError> {
        // 生成 Gaia 程序
        let gaia_program = self.compile_to_gaia(source)?;

        // 使用 gaia-assembler 编译到目标平台
        let assembler = GaiaAssembler::new();
        let generated_files = assembler.compile(&gaia_program, &target)?;

        // 返回主要的二进制文件
        if let Some((_, bytes)) = generated_files.files.iter().next() {
            Ok(bytes.clone())
        }
        else {
            Err(GaiaError::invalid_data("No output files generated"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mini_c_frontend() {
        let source = r#"
int add(int a, int b) {
    return a + b;
}

char* message = "Hello, World!";
int count = 42;
"#;

        let config = ReadConfig::new();
        let lexer = CLexer::new(&config);
        let token_stream = lexer.tokenize(source).result.unwrap();

        let mut parser = CParser::new(token_stream);
        let program = parser.parse().unwrap();

        assert_eq!(program.declarations.len(), 3);
    }

    #[test]
    fn test_compile_to_gaia() {
        let code = r#"
int main() {
    return 0;
}
"#;
        let mut frontend = MiniCFrontend::new();
        let result = frontend.compile_to_gaia(code);
        assert!(result.is_ok());
    }
}
