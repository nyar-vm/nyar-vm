//! Rusty Swift 语言前端
//!
//! 这个库提供了 Rusty Swift 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::source::SourceText;
use oak_swift::{SwiftBuilder, SwiftLanguage};
use oak_core::Builder;

/// Rusty Swift 前端
pub struct RustySwiftFrontend {
    language: SwiftLanguage,
}

impl Default for RustySwiftFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustySwiftFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: SwiftLanguage::default(),
        }
    }
}

impl NyarFrontend for RustySwiftFrontend {
    type Language = SwiftLanguage;

    fn parse(&self, source: &str) -> Result<(), NyarError> {
        let builder = SwiftBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<SwiftLanguage>::default();

        let _output = builder.build(&source_text, &[], &mut session);
        Ok(())
    }

    fn lower(&self, _ast: &()) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 AST 到 IKunTree 的转换
        Ok(IKunTree::Module("rusty-swift-program".to_string(), Vec::new()))
    }
}
