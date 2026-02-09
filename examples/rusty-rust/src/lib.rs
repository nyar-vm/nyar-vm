//! Mini Rust 语言实现
#![feature(new_range_api)]

pub mod ast;
// pub mod codegen;
pub mod converter;

use chomsky_uir::ConstraintAnalysis;
use nyar_types::{IKunTree, NyarError, NyarFrontend, NyarContext, Id, IKun};
use oak_vfs::Vfs;
use oak_core::source::SourceText;
use oak_core::Builder;
use oak_rust::{RustBuilder, RustLanguage, RustRoot};

/// Mini Rust 前端实现
#[derive(Default)]
pub struct MiniRustFrontend {
    language: RustLanguage,
}

impl MiniRustFrontend {
    /// 创建一个新的 Mini Rust 前端
    pub fn new() -> Self {
        Self {
            language: RustLanguage::default(),
        }
    }
}

impl NyarFrontend<ConstraintAnalysis> for MiniRustFrontend {
    type Language = RustLanguage;

    fn parse(&self, source: &str) -> Result<RustRoot, NyarError> {
        let builder = RustBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<RustLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &RustRoot, ctx: &mut NyarContext<V, ConstraintAnalysis>) -> Id {
        let mut builder = ctx.builder();
        converter::convert_root(ast, &mut builder)
    }

    fn lower<V: Vfs>(&self, _ast: &RustRoot, _vfs: &V) -> Result<IKunTree, NyarError> {
        let mut egraph = chomsky_uir::EGraph::<IKun, ConstraintAnalysis>::new();
        let mut builder = chomsky_uir::IntentBuilder::new(&mut egraph);
        let id = converter::convert_root(_ast, &mut builder);

        egraph.rebuild();

        let extractor = chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
        let tree = extractor.extract(id);

        Ok(tree)
    }
}
