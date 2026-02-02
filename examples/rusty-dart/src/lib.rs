//! Mini Dart 语言前端
//!
//! 这个库提供了 Mini Dart 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_dart::{DartBuilder, DartLanguage, DartRoot};

/// Mini Dart 前端
pub struct MiniDartFrontend {
    language: DartLanguage,
}

impl Default for MiniDartFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniDartFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: DartLanguage {},
        }
    }
}

impl NyarFrontend for MiniDartFrontend {
    type Language = DartLanguage;

    fn parse(&self, source: &str) -> Result<DartRoot, NyarError> {
        let builder = DartBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<DartLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &DartRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 DartRoot 到 IKunTree 的转换
        Ok(IKunTree::Module("mini-dart-program".to_string(), Vec::new()))
    }
}
