//! Rusty Zig 语言前端
//!
//! 这个库提供了 Rusty Zig 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_zig::ZigLanguage;

/// Rusty Zig 前端
pub struct RustyZigFrontend {
    language: ZigLanguage,
}

impl Default for RustyZigFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyZigFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: ZigLanguage::default(),
        }
    }
}

impl NyarFrontend for RustyZigFrontend {
    type Language = ZigLanguage;

    fn parse(&self, _source: &str) -> Result<(), NyarError> {
        // TODO: 实现真正的解析逻辑
        Ok(())
    }

    fn lower(&self, _ast: &()) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 AST 到 IKunTree 的转换
        Ok(IKunTree::Module("rusty-zig-program".to_string(), Vec::new()))
    }
}
