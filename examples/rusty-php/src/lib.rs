//! Mini PHP 语言前端
//!
//! 这个库提供了 Mini PHP 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_php::{PhpBuilder, PhpLanguage, PhpRoot};

/// Mini PHP 前端
pub struct MiniPhpFrontend {
    language: PhpLanguage,
}

impl Default for MiniPhpFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniPhpFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: PhpLanguage {},
        }
    }
}

impl NyarFrontend for MiniPhpFrontend {
    type Language = PhpLanguage;

    fn parse(&self, source: &str) -> Result<PhpRoot, NyarError> {
        let builder = PhpBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<PhpLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &PhpRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 PhpRoot 到 IKunTree 的转换
        Ok(IKunTree::Module("mini-php-program".to_string(), Vec::new()))
    }
}
