//! Mini Java 语言前端
//!
//! 提供 Mini Java 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;

use oak_java::{JavaLanguage, parser::JavaParser, ast::JavaRoot};
use oak_core::source::Source;
use nyar_vm::bytecode::format::NyarModule;
use anyhow::Result;

/// Mini Java 前端
pub struct MiniJavaFrontend {
    language: JavaLanguage,
}

impl MiniJavaFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: JavaLanguage::default(),
        }
    }

    /// 解析 Java 源代码
    pub fn parse(&self, source: &str) -> Result<JavaRoot> {
        // 这里只是示意，实际解析逻辑需要根据 oak-java 的实现来调用
        // 假设 JavaParser 有一个简单的接口
        // let mut parser = JavaParser::new(&self.language);
        // parser.parse(source)
        
        // 由于 oak-java 目前可能还是骨架，我们先返回一个空的 Root
        Ok(JavaRoot { items: vec![] })
    }

    /// 编译到 Nyar 字节码
    pub fn compile_to_nyar(&self, source: &str) -> Result<NyarModule> {
        let ast = self.parse(source)?;
        let translator = codegen::NyarTranslator::new();
        translator.translate(&ast)
    }
}

impl Default for MiniJavaFrontend {
    fn default() -> Self {
        Self::new()
    }
}
