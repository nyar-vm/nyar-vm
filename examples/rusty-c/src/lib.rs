#![feature(new_range_api)]
//! Mini C 解释器
//!
//! 基于 Oaks (前端), Chomsky (优化), Gaia (后端) 和 Nyar VM (运行时) 架构实现。

pub mod frontend;
pub mod optimizer;
pub mod runtime;

use nyar_types::{NyarError, NyarFrontend, IKunTree};
use oak_c::{CLanguage, CRoot, CBuilder};
use oak_core::source::SourceText;

/// Mini C 前端实现
#[derive(Default)]
pub struct MiniCFrontend {
    language: CLanguage,
}

impl MiniCFrontend {
    /// 创建一个新的 Mini C 前端
    pub fn new() -> Self {
        Self {
            language: CLanguage::default(),
        }
    }
}

impl NyarFrontend for MiniCFrontend {
    type Language = CLanguage;

    fn parse(&self, source: &str) -> Result<CRoot, NyarError> {
        let builder = CBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<CLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &CRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现从 CRoot 到 IKunTree 的转换
        let mut tree = IKunTree::default();
        tree.name = "mini-c-program".to_string();
        Ok(tree)
    }
}
