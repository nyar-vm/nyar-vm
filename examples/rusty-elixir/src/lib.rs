//! Rusty Elixir 语言前端
//!
//! 这个库提供了 Rusty Elixir 语言的词法分析、语法分析和 Gaia 翻译功能。

#![feature(new_range_api)]

pub mod codegen;

use nyar_types::{NyarContext, NyarError, NyarFrontend};
use oak_elixir::{ElixirLanguage, ElixirRoot};
use oak_vfs::Vfs;
use chomsky_uir::Id;

/// Rusty Elixir 前端
pub struct RustyElixirFrontend {
    language: ElixirLanguage,
}

impl Default for RustyElixirFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyElixirFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: ElixirLanguage::default(),
        }
    }
}

impl NyarFrontend for RustyElixirFrontend {
    type Language = ElixirLanguage;

    fn parse(&self, _source: &str) -> Result<ElixirRoot, NyarError> {
        // TODO: 实现真正的解析逻辑
        // 目前返回一个空的根节点
        Ok(ElixirRoot { items: Vec::new() })
    }

    fn lower_unified<V: Vfs>(&self, ast: &ElixirRoot, ctx: &mut NyarContext<V>) -> Id {
        codegen::lower_elixir_root(ast, ctx)
    }
}
