#![feature(new_range_api)]
//! Mini CSharp 语言前端
//!
//! 提供 Mini CSharp 的词法分析、语法分析和 Nyar 翻译功能。

use chomsky_extract::IKunTree;
use nyar_types::{NyarError, NyarFrontend};
use oak_core::{builder::Builder, source::SourceText};
use oak_java::{JavaBuilder, JavaLanguage, JavaRoot};

pub mod codegen;
pub mod errors;
pub mod row_type;
pub mod tagless;
pub mod visitor;

/// Mini CSharp 前端
#[derive(Default)]
pub struct MiniCSharpFrontend {
    language: JavaLanguage,
    builder: JavaBuilder,
}

impl MiniCSharpFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self::default()
    }
}

impl NyarFrontend for MiniCSharpFrontend {
    type Language = JavaLanguage;

    /// 解析 CSharp 源代码
    fn parse(&self, source: &str) -> Result<JavaRoot, NyarError> {
        let mut session = oak_core::parser::ParseSession::<JavaLanguage>::default();
        let source_text = SourceText::new(source);
        let output = self.builder.build(&source_text, &[], &mut session);
        match output.result {
            Ok(root) => Ok(root),
            Err(e) => Err(NyarError::Parse(format!("{:?}", e))),
        }
    }

    /// 编译到 Chomsky UIR (IKunTree)
    fn lower(&self, ast: &JavaRoot) -> Result<IKunTree, NyarError> {
        let translator = codegen::NyarTranslator::new();
        translator.translate_to_tree(ast).map_err(|e| NyarError::Compile(e.to_string()))
    }
}

impl Default for MiniCSharpFrontend {
    fn default() -> Self {
        Self::new()
    }
}
