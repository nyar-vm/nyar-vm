#![feature(new_range_api)]
//! Mini Go 解释器

pub mod frontend;
pub mod optimizer;
pub mod runtime;

use nyar_types::{NyarError, NyarFrontend, IKunTree};
use crate::frontend::MiniGoFrontend as FrontendImpl;
use crate::optimizer::MiniGoOptimizer;
use crate::runtime::MiniGoRuntime;
use oak_c::CLanguage;

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
    type Language = CLanguage;

    fn parse(&self, source: &str) -> Result<oak_c::CRoot, NyarError> {
        use oak_core::Builder;
        let builder = oak_c::CBuilder::new(oak_c::CLanguage::default());
        let source_text = oak_core::source::SourceText::new(source.to_string());
        let mut cache = oak_core::parser::session::ParseSession::<CLanguage>::default();
        let output = builder.build(&source_text, &[], &mut cache);
        output.result.map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, ast: &oak_c::CRoot) -> Result<IKunTree, NyarError> {
        self.inner.lower(ast)
    }
}
