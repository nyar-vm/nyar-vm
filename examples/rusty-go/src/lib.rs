#![feature(new_range_api)]
//! Rusty Go 解释器

pub mod frontend;
pub mod optimizer;
pub mod runtime;

use crate::frontend::RustyGoFrontend as FrontendImpl;
use crate::optimizer::RustyGoOptimizer;
use crate::runtime::RustyGoRuntime;
use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_go::{GoBuilder, GoLanguage, GoRoot};

/// Rusty Go 前端实现
#[derive(Default)]
pub struct RustyGoFrontend {
    inner: FrontendImpl,
}

impl RustyGoFrontend {
    /// 创建一个新的 Rusty Go 前端
    pub fn new() -> Self {
        Self {
            inner: FrontendImpl::default(),
        }
    }
}

impl NyarFrontend for RustyGoFrontend {
    type Language = GoLanguage;

    fn parse(&self, source: &str) -> Result<GoRoot, NyarError> {
        use oak_core::Builder;
        let config = GoLanguage::default();
        let builder = GoBuilder::new(&config);
        let source_text = oak_core::source::SourceText::new(source.to_string());
        let mut cache = oak_core::parser::session::ParseSession::<GoLanguage>::default();
        let output = builder.build(&source_text, &[], &mut cache);
        output
            .result
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower(&self, ast: &GoRoot) -> Result<IKunTree, NyarError> {
        self.inner.lower(ast)
    }
}
