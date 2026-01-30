#![feature(new_range_api)]
//! Mini Go 解释器

pub mod frontend;
pub mod optimizer;
pub mod runtime;

use nyar_types::{NyarError, NyarFrontend, IKunTree};
use oak_c::{CLanguage, CRoot, CBuilder}; // Currently using oak_c as placeholder
use oak_core::source::SourceText;

/// Mini Go 前端实现
#[derive(Default)]
pub struct MiniGoFrontend {
    language: CLanguage,
}

impl MiniGoFrontend {
    /// 创建一个新的 Mini Go 前端
    pub fn new() -> Self {
        Self {
            language: CLanguage::default(),
        }
    }
}

impl NyarFrontend for MiniGoFrontend {
    type Language = CLanguage;

    fn parse(&self, source: &str) -> Result<CRoot, NyarError> {
        let builder = CBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<CLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &CRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现从 AST 到 IKunTree 的转换
        let mut tree = IKunTree::default();
        tree.name = "mini-go-program".to_string();
        Ok(tree)
    }
}
