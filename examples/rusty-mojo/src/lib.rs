//! Mini Mojo 语言前端
//!
//! 这个库提供了 Mini Mojo 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::language::PlaceholderLanguage;

/// Mini Mojo 前端
pub struct MiniMojoFrontend {
    language: PlaceholderLanguage,
}

impl Default for MiniMojoFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniMojoFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: MojoLanguage::default(),
        }
    }
}

impl NyarFrontend for MiniMojoFrontend {
    type Language = PlaceholderLanguage;

    fn parse(&self, _source: &str) -> Result<(), NyarError> {
        Err(NyarError::Parse("Mojo parser not yet implemented".to_string()))
    }

    fn lower(&self, _ast: &()) -> Result<IKunTree, NyarError> {
        Ok(IKunTree::Module("mini-mojo-program".to_string(), Vec::new()))
    }
}
