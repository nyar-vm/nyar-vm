//! Mini Rust 语言实现
#![feature(new_range_api)]

pub mod ast;
pub mod codegen;
pub mod converter;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::source::SourceText;
use oak_rust::{RustBuilder, RustLanguage, RustRoot};
use chomsky_uir::IntentBuilder;
use chomsky_uir::ConstraintAnalysis;

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

impl NyarFrontend for MiniRustFrontend {
    type Language = RustLanguage;

    fn parse(&self, source: &str) -> Result<RustRoot, NyarError> {
        let builder = RustBuilder::new(&self.language);
        let source_text = SourceText::new(source);
        let mut session = oak_core::parser::ParseSession::<RustLanguage>::default();

        let output = builder.build(&source_text, &[], &mut session);
        output.result.map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, _ast: &RustRoot) -> Result<IKunTree, NyarError> {
        let mut builder = chomsky_uir::IntentBuilder::<chomsky_uir::ConstraintAnalysis>::new();
        let id = converter::convert_root(_ast, &mut builder);
        let intent = builder.finish(id);

        let mut aot = nyar_aot::NyarAot::new();
        let backend = nyar_vm::bytecode::compiler::NyarBackend::new();
        
        let artifact = aot.compile(&intent, &backend)
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))?;
            
        // NyarBackend::generate returns BackendArtifact::Source(json) which is NyarModule
        // But we want the IKunTree itself if possible, or we just return a stub and 
        // handle the real compilation in driver.
        
        // Actually, NyarAot::compile extracts the tree internally. 
        // I might need a way to just get the tree.
        
        let id = aot.optimizer.add_intent(&intent);
        aot.optimizer.saturate();
        let tree = aot.optimizer.extract(id, backend.get_model());
        
        Ok(tree)
    }
}
