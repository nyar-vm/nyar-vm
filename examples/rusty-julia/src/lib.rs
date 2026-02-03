//! Rusty Julia 语言前端
//!
//! 这个库提供了 Rusty Julia 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_julia::{JuliaBuilder, JuliaLanguage, JuliaRoot};

/// Rusty Julia 前端
pub struct RustyJuliaFrontend {
    language: JuliaLanguage,
}

impl Default for RustyJuliaFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyJuliaFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: JuliaLanguage::default(),
        }
    }
}

impl NyarFrontend for RustyJuliaFrontend {
    type Language = JuliaLanguage;

    fn parse(&self, source: &str) -> Result<JuliaRoot, NyarError> {
        let builder = JuliaBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<JuliaLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &JuliaRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 JuliaRoot 到 IKunTree 的转换
        Ok(IKunTree::Module("rusty-julia-program".to_string(), Vec::new()))
    }
}
