#![feature(new_range_api)]
//! Mini Go 解释器

pub mod frontend;
pub mod optimizer;
pub mod runtime;

use crate::frontend::MiniGoFrontend as FrontendImpl;
use crate::optimizer::MiniGoOptimizer;
use crate::runtime::MiniGoRuntime;
use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_go::{GoLanguage, GoRoot, GoBuilder};

/// Mini Go 前端实现
#[derive(Default)]
pub struct MiniGoFrontend {
    inner: FrontendImpl,
}

impl MiniGoFrontend {
    /// 创建一个新的 Mini Go 前端
    pub fn new() -> Self {
        Self {
            inner: FrontendImpl::new(),
        }
    }
}

impl NyarFrontend for MiniGoFrontend {
    type Language = GoLanguage;

    fn parse(&self, source: &str) -> Result<GoRoot, NyarError> {
        use oak_core::Builder;
        let builder = GoBuilder::new(GoLanguage::default());
        let source_text = oak_core::source::SourceText::new(source.to_string());
        let mut cache = oak_core::parser::session::ParseSession::<GoLanguage>::default();
        let output = builder.build(&source_text, &[], &mut cache);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, ast: &GoRoot) -> Result<IKunTree, NyarError> {
        self.inner.lower(ast)
    }
}
