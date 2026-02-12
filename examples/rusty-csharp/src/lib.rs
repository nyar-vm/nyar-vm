#![feature(new_range_api)]
//! Rusty CSharp 语言前端
//!
//! 提供 Rusty CSharp 的词法分析、语法分析和 Nyar 翻译功能。

use nyar_aot::{NyarContext, NyarFrontend};
use nyar_types::NyarError;
use oak_core::{builder::Builder, source::SourceText};
use oak_csharp::{ast::CSharpRoot, CSharpBuilder, CSharpLanguage};
use oak_vfs::Vfs;

pub mod codegen;
pub mod errors;
pub mod row_type;
pub mod tagless;
pub mod visitor;
pub mod runtime;

pub use crate::runtime::RustyCSharpRuntime;

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

impl NyarFrontend<()> for RustyCSharpFrontend {
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

    fn lower_unified<V: Vfs>(
        &self,
        ast: &CSharpRoot,
        ctx: &mut NyarContext<'_, V, ()>,
    ) -> chomsky_uir::Id {
        let translator = crate::codegen::NyarTranslator::new();
        let mut translator_ctx = crate::codegen::TranslatorContext::new_with_builder(ctx.builder());
        translator.translate_root(ast, &mut translator_ctx).unwrap_or_else(|_| ctx.egraph.add(chomsky_uir::IKun::Seq(vec![])))
    }
}
