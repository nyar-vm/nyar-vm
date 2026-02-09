//! Rusty FSharp 语言前端
//!
//! 这个库提供了 Rusty FSharp 语言的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{source::SourceText, Builder};
use oak_fsharp::{FSharpLanguage};

/// Rusty FSharp 前端
pub struct RustyFSharpFrontend {
    language: FSharpLanguage,
}

impl Default for RustyFSharpFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyFSharpFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: FSharpLanguage::new(),
        }
    }
}

impl NyarFrontend for RustyFSharpFrontend {
    type Language = FSharpLanguage;

    fn parse(&self, source: &str) -> Result<oak_core::GreenNode<FSharpLanguage>, NyarError> {
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<FSharpLanguage>::default();
        let parser = oak_fsharp::parser::FSharpParser::new(&self.language);
        let mut cache = oak_core::parser::ParseCacheImpl::default();
        let output = parser.parse(&source_text, &[], &mut cache);
        
        output.result
            .map(|node| node.clone())
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &oak_core::GreenNode<FSharpLanguage>) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 FSharp GreenNode 到 IKunTree 的转换
        Ok(IKunTree::Module("rusty-fsharp-program".to_string(), Vec::new()))
    }
}
