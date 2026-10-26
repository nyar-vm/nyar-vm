//! Rusty TypeScript 前端（解析与 Nyar 接入）

use chomsky_cost;
use chomsky_emit::GaiaEmitter;
use chomsky_extract::{Backend, IKunExtractor};
use chomsky_types::Loc;
use chomsky_uir::{Analysis, ConstraintAnalysis, EGraph, IKun, IKunTree, Id, IntentBuilder};
use nyar_aot::{NyarContext, NyarFrontend};
use nyar_types::NyarError;
use odin_vm::bytecode::format::NyarcModule;
use oak_core::{ParseSession, SourceText};
use oak_typescript::{ast, TypeScriptBuilder, TypeScriptLanguage, TypeScriptRoot};
use oak_vfs::Vfs;

use super::frontend_emit::UirConverter;

/// Rusty TypeScript 前端
pub struct RustyTypescriptFrontend {
    language: TypeScriptLanguage,
    source_id: u32,
}

impl Default for RustyTypescriptFrontend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
wit_bindgen::generate!({
    world: "compiler",
    path: "wit",
});

#[cfg(target_arch = "wasm32")]
struct Compiler;

#[cfg(target_arch = "wasm32")]
impl Guest for Compiler {
    fn compile(source: String) -> Result<Vec<u8>, String> {
        let frontend = RustyTypescriptFrontend::new();
        let artifacts = frontend.compile_to_wasm(&source)?;

        // Return main.wasm or the first artifact
        if let Some(wasm) = artifacts.get("main.wasm") {
            Ok(wasm.clone())
        }
        else if let Some((_, bytes)) = artifacts.iter().next() {
            Ok(bytes.clone())
        }
        else {
            Err("No output files generated".to_string())
        }
    }
}

#[cfg(target_arch = "wasm32")]
export!(Compiler);

impl RustyTypescriptFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let mut language = TypeScriptLanguage::standard();
        language.decorators = true;
        language.jsx = true;
        Self {
            language,
            source_id: 1, // 默认 source_id
        }
    }

    /// 设置当前处理的源码 ID
    pub fn set_source_id(&mut self, id: u32) {
        self.source_id = id;
    }

    /// 编译源码为 WASM (AOT)
    pub fn compile_to_wasm(&self, source: &str) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
        let tree = self.lower_to_tree(source)?;

        let emitter = GaiaEmitter::new("wasm32-wasi").standalone();

        // 使用 AOT 编译器进行优化和生成
        // 由于 GaiaEmitter 实现了 Backend 接口，可以直接调用 generate
        let artifact = emitter.generate(&tree).map_err(|e| format!("AOT error: {:?}", e))?;

        match artifact {
            chomsky_extract::BackendArtifact::Binary(bytes) => {
                let mut files = std::collections::HashMap::new();
                files.insert("main.wasm".to_string(), bytes);
                Ok(files)
            }
            chomsky_extract::BackendArtifact::Collection(files) => Ok(files),
            _ => Err("Expected binary artifact from AOT compiler".to_string()),
        }
    }

    fn lower_to_tree(&self, source: &str) -> Result<IKunTree, String> {
        let ast = self.parse(source).map_err(|e| format!("Parse error: {:?}", e))?;
        let vfs = oak_vfs::MemoryVfs::new();
        self.lower(&ast, &vfs).map_err(|e| format!("Lowering error: {:?}", e))
    }

    /// 编译源码为 Nyar 模块
    pub fn compile_to_nyar(&self, source: &str) -> Result<NyarcModule, String> {
        let tree = self.lower_to_tree(source)?;

        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = tree.to_egraph(&mut egraph);

        let mut translator = codegen::NyarTranslator::new();
        translator.generate(&egraph, root_id).map_err(|e| format!("Codegen error: {:?}", e))
    }
}

impl NyarFrontend<()> for RustyTypescriptFrontend {
    type Language = TypeScriptLanguage;

    fn parse(&self, source: &str) -> Result<TypeScriptRoot, NyarError> {
        let builder = TypeScriptBuilder::new(&self.language);
        let mut session = ParseSession::<TypeScriptLanguage>::default();
        let source_text = SourceText::new(source);
        let output = oak_core::Builder::build(&builder, &source_text, &[], &mut session);

        output.result.map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    fn lower_unified<V: Vfs>(&self, ast: &TypeScriptRoot, ctx: &mut NyarContext<V, ()>) -> Id {
        let source_id = self.source_id;
        let mut builder = ctx.builder();
        let mut converter = UirConverter::new(&mut builder, source_id);
        converter.convert_root(ast.clone())
    }

    fn lower<V: oak_vfs::Vfs>(&self, ast: &TypeScriptRoot, _vfs: &V) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let mut builder = IntentBuilder::new(&mut egraph);
        let mut converter = UirConverter::new(&mut builder, self.source_id);

        let root_id = converter.convert_root(ast.clone());
        let extractor = IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
        let tree = extractor.extract(root_id);
        Ok(tree)
    }
}
