//! Mini Elixir 语言前端
//!
//! 这个库提供了 Mini Elixir 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_elixir::{ElixirLanguage, ElixirRoot};

/// Mini Elixir 前端
pub struct MiniElixirFrontend {
    language: ElixirLanguage,
}

impl Default for MiniElixirFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniElixirFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: ElixirLanguage {},
        }
    }
}

impl NyarFrontend for MiniElixirFrontend {
    type Language = ElixirLanguage;

    fn parse(&self, _source: &str) -> Result<ElixirRoot, NyarError> {
        // TODO: 实现真正的解析逻辑
        Err(NyarError::Parse("Elixir parser not yet integrated".to_string()))
    }

    fn lower(&self, _ast: &ElixirRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 ElixirRoot 到 IKunTree 的转换
        Ok(IKunTree::Module("mini-elixir-program".to_string(), Vec::new()))
    }
}
