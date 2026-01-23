//! Mini TypeScript 语言前端
//!
//! 这个库提供了 Mini TypeScript 语言的解析和 Nyar 翻译功能。
//! 遵循 Project Chomsky Whitebook 规范。

pub mod codegen;
pub mod project;

use oak_typescript::TypeScriptParser;
use codegen::NyarTranslator;
use nyar_vm::bytecode::format::NyarModule;
use nyar_error::FormatError;
use chomsky_uir::{EGraph, Id};

/// Mini TypeScript 前端
pub struct MiniTypescriptFrontend {
    translator: NyarTranslator,
}

impl MiniTypescriptFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self { translator: NyarTranslator::new() }
    }

    /// 解析 TypeScript 源代码为 UIR
    pub fn parse(&mut self, source: &str) -> Result<(EGraph, Id), String> {
        let mut parser = TypeScriptParser::new(source);
        parser.parse_module().map_err(|e| format!("Parse error: {:?}", e))
    }

    /// 将 TypeScript 源代码编译为 Nyar 程序
    pub fn compile_to_nyar(&mut self, source: &str) -> Result<NyarModule, FormatError> {
        // 解析为 UIR
        let (egraph, root) = self.parse(source).map_err(|_e| {
            // 这里我们暂时简单地返回一个 FormatError
            // 实际上应该有更好的错误转换
            FormatError::InvalidHeader
        })?;

        // 翻译为 Nyar 程序
        self.translator.generate(&egraph, root)
    }

    /// 仅进行词法分析
    pub fn tokenize(&mut self, source: &str) -> Result<Vec<oak_typescript::TokenInfo>, String> {
        let mut lexer = oak_typescript::TypeScriptLexer::new(source);
        let mut tokens = Vec::new();
        loop {
            let token_info = lexer.next_token();
            if token_info.token == oak_typescript::Token::EOF {
                break;
            }
            tokens.push(token_info);
        }
        Ok(tokens)
    }

    /// 获取翻译器的可变引用
    pub fn translator_mut(&mut self) -> &mut NyarTranslator {
        &mut self.translator
    }

    /// 获取翻译器的不可变引用
    pub fn translator(&self) -> &NyarTranslator {
        &self.translator
    }
}
