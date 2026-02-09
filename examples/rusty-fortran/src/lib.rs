//! Rusty Fortran 语言前端
//!
//! 这个库提供了 Rusty Fortran 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{NyarContext, NyarError, NyarFrontend, Vfs};
use oak_core::source::SourceText;
use oak_fortran::{FortranBuilder, FortranLanguage, ast::FortranRoot};
use chomsky_uir::Id;

/// Rusty Fortran 前端
#[derive(Default)]
pub struct RustyFortranFrontend {
    language: FortranLanguage,
}

impl RustyFortranFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: FortranLanguage::new(),
        }
    }
}

impl NyarFrontend for RustyFortranFrontend {
    type Language = FortranLanguage;

    fn parse(&self, source: &str) -> Result<FortranRoot, NyarError> {
        let mut session = oak_core::parser::ParseSession::<FortranLanguage>::default();
        let source_text = SourceText::new(source);
        let builder = FortranBuilder::new(&self.language);
        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &FortranRoot, ctx: &mut NyarContext<V>) -> Id {
        // TODO: 实现真正的从 FortranRoot 到 IKunTree 的转换
        ctx.builder().module("rusty-fortran-program", vec![])
    }
}
