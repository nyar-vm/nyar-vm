#![feature(new_range_api)]
//! Mini CSharp 语言前端
//!
//! 提供 Mini CSharp 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;
pub mod visitor;
pub mod tagless;
pub mod wasm;
pub mod row_type;
pub mod errors;

use oak_java::{JavaLanguage, JavaRoot, JavaBuilder};
use oak_core::{source::SourceText, builder::Builder};
use nyar_vm::bytecode::format::NyarModule;
use chomsky_uir::{EGraph, IKun, ConstraintAnalysis};
use std::fmt::{Display, Formatter};
use std::error::Error;

#[derive(Debug)]
pub enum CSharpError {
    Parse(String),
    Codegen(String),
    Other(String),
}

impl Display for CSharpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CSharpError::Parse(msg) => write!(f, "CSharp parse error: {}", msg),
            CSharpError::Codegen(msg) => write!(f, "CSharp codegen error: {}", msg),
            CSharpError::Other(msg) => write!(f, "CSharp error: {}", msg),
        }
    }
}

impl Error for CSharpError {}

impl From<String> for CSharpError {
    fn from(s: String) -> Self {
        CSharpError::Other(s)
    }
}

impl From<&str> for CSharpError {
    fn from(s: &str) -> Self {
        CSharpError::Other(s.to_string())
    }
}

pub type CSharpResult<T> = Result<T, CSharpError>;

/// Mini CSharp 前端
pub struct MiniCSharpFrontend {
    language: JavaLanguage,
    builder: JavaBuilder,
}

impl MiniCSharpFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let language = JavaLanguage::default();
        let builder = JavaBuilder::new(language.clone());
        Self { language, builder }
    }

    /// 解析 CSharp 源代码
    pub fn parse(&self, source: &str) -> CSharpResult<JavaRoot> {
        let mut session = oak_core::parser::ParseSession::<JavaLanguage>::default();
        let source_text = SourceText::new(source);
        let output = self.builder.build(&source_text, &[], &mut session);
        match output.result {
            Ok(root) => Ok(root),
            Err(e) => Err(CSharpError::Parse(format!("{:?}", e))),
        }
    }

    /// 编译到 Chomsky UIR (EGraph)
    pub fn compile_to_uir(&self, source: &str) -> CSharpResult<EGraph<IKun, ConstraintAnalysis>> {
        let ast = self.parse(source)?;
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let translator = codegen::NyarTranslator::new();
        translator.translate_to_graph(&ast, &mut egraph)?;
        Ok(egraph)
    }

    /// 编译到 Nyar 字节码
    pub fn compile_to_nyar(&self, source: &str) -> CSharpResult<NyarModule> {
        let ast = self.parse(source)?;
        let translator = codegen::NyarTranslator::new();
        translator.translate(&ast)
    }

    /// 从源代码生成后端产物
    pub fn generate_from_source(&self, source: &str) -> CSharpResult<chomsky_extract::BackendArtifact> {
        let module = self.compile_to_nyar(source)?;
        let data = module.encode();
        Ok(chomsky_extract::BackendArtifact::Binary(data))
    }
}

impl Default for MiniCSharpFrontend {
    fn default() -> Self {
        Self::new()
    }
}
