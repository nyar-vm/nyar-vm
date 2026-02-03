//! Rusty Mojo 语言前端
//!
//! 这个库提供了 Rusty Mojo 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::language::PlaceholderLanguage;

/// Rusty Mojo 前端
pub struct RustyMojoFrontend {
    language: PlaceholderLanguage,
}

impl Default for RustyMojoFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyMojoFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: PlaceholderLanguage::default(),
        }
    }
}

impl NyarFrontend for RustyMojoFrontend {
    type Language = PlaceholderLanguage;

    fn parse(&self, _source: &str) -> Result<(), NyarError> {
        Err(NyarError::Parse("Mojo parser not yet implemented".to_string()))
    }

    fn lower(&self, _ast: &()) -> Result<IKunTree, NyarError> {
        Ok(IKunTree::Module("rusty-mojo-program".to_string(), Vec::new()))
    }
}
