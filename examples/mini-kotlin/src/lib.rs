#![feature(new_range_api)]
//! Mini Kotlin 语言前端
//!
//! 提供 Mini Kotlin 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;
pub mod visitor;
pub mod tagless;
pub mod wasm;
pub mod row_type;
pub mod errors;

use oak_kotlin::{KotlinLanguage, KotlinRoot, KotlinBuilder};
use oak_core::{source::SourceText, builder::Builder};
use nyar_vm::bytecode::format::NyarModule;
use chomsky_uir::{EGraph, IKun, ConstraintAnalysis};
use std::fmt::{Display, Formatter};
use std::error::Error;

#[derive(Debug)]
pub enum KotlinError {
    Parse(String),
    Codegen(String),
    Other(String),
}

impl Display for KotlinError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            KotlinError::Parse(msg) => write!(f, "Kotlin parse error: {}", msg),
            KotlinError::Codegen(msg) => write!(f, "Kotlin codegen error: {}", msg),
            KotlinError::Other(msg) => write!(f, "Kotlin error: {}", msg),
        }
    }
}

impl Error for KotlinError {}

impl From<String> for KotlinError {
    fn from(s: String) -> Self {
        KotlinError::Other(s)
    }
}

impl From<&str> for KotlinError {
    fn from(s: &str) -> Self {
        KotlinError::Other(s.to_string())
    }
}

pub type KotlinResult<T> = Result<T, KotlinError>;

/// Mini Kotlin 前端
pub struct MiniKotlinFrontend {
    language: KotlinLanguage,
    builder: KotlinBuilder<'static>,
}

impl MiniKotlinFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let language = Box::leak(Box::new(KotlinLanguage::default()));
        let builder = KotlinBuilder::new(language);
        Self { language: language.clone(), builder }
    }

    /// 解析 Kotlin 源代码
    pub fn parse(&self, source: &str) -> KotlinResult<KotlinRoot> {
        let mut session = oak_core::parser::ParseSession::<KotlinLanguage>::default();
        let source_text = SourceText::new(source);
        let output = self.builder.build(&source_text, &[], &mut session);
        match output.result {
            Ok(root) => Ok(root),
            Err(e) => Err(KotlinError::Parse(format!("{:?}", e))),
        }
    }

    /// 编译到 Chomsky UIR (EGraph)
    pub fn compile_to_uir(&self, source: &str) -> KotlinResult<EGraph<IKun, ConstraintAnalysis>> {
        let ast = self.parse(source)?;
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let translator = codegen::NyarTranslator::new();
        translator.translate_to_graph(&ast, &mut egraph)?;
        Ok(egraph)
    }

    /// 编译到 Nyar 字节码
    pub fn compile_to_nyar(&self, source: &str) -> KotlinResult<NyarModule> {
        let ast = self.parse(source)?;
        let translator = codegen::NyarTranslator::new();
        translator.translate(&ast)
    }

    /// 从源代码生成后端产物
    pub fn generate_from_source(&self, source: &str) -> KotlinResult<chomsky_extract::BackendArtifact> {
        let module = self.compile_to_nyar(source)?;
        let data = module.encode();
        Ok(chomsky_extract::BackendArtifact::Binary(data))
    }
}
