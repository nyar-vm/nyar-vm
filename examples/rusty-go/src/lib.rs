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
        // This is a bit of a hack since FrontendImpl::parse returns EGraph
        // But for now let's just make it compile
        Err(NyarError::Parse("Not implemented yet".to_string()))
    }

    fn lower(&self, _ast: &oak_c::CRoot) -> Result<IKunTree, NyarError> {
        let tree = IKunTree::Module("mini-go-program".to_string(), vec![]);
        Ok(tree)
    }
}
