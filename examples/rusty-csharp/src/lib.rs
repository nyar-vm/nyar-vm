#![feature(new_range_api)]
//! Rusty CSharp 语言前端
//!
//! 提供 Rusty CSharp 的词法分析、语法分析和 Nyar 翻译功能。

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{builder::Builder, source::SourceText};
use oak_csharp::{CSharpBuilder, CSharpLanguage, ast::CSharpRoot};

pub mod codegen;
pub mod errors;
pub mod row_type;
pub mod tagless;
pub mod visitor;

/// Rusty CSharp 前端
#[derive(Default)]
pub struct RustyCSharpFrontend {
    language: CSharpLanguage,
}

impl RustyCSharpFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: CSharpLanguage::new(),
        }
    }
}

impl NyarFrontend for RustyCSharpFrontend {
    type Language = CSharpLanguage;

    /// 解析 CSharp 源代码
    fn parse(&self, source: &str) -> Result<CSharpRoot, NyarError> {
        let mut session = oak_core::parser::ParseSession::<CSharpLanguage>::default();
        let source_text = SourceText::new(source);
        let builder = CSharpBuilder::new(&self.language);
        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    /// 编译到 Chomsky UIR (IKunTree)
    fn lower(&self, ast: &CSharpRoot) -> Result<IKunTree, NyarError> {
        let translator = crate::codegen::NyarTranslator::new();
        translator.translate_to_tree(ast)
    }
}
