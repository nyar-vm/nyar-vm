//! Mini Ruby 语言前端
//!
//! 这个库提供了 Mini Ruby 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_ruby::{ast::RubyAst, RubyBuilder, RubyLanguage};

/// Mini Ruby 前端
pub struct MiniRubyFrontend {
    language: RubyLanguage,
}

impl Default for MiniRubyFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniRubyFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: RubyLanguage {},
        }
    }
}

impl NyarFrontend for MiniRubyFrontend {
    type Language = RubyLanguage;

    fn parse(&self, source: &str) -> Result<RubyAst, NyarError> {
        let builder = RubyBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<RubyLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &RubyAst) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 RubyAst 到 IKunTree 的转换
        Ok(IKunTree::Module("mini-ruby-program".to_string(), Vec::new()))
    }
}
