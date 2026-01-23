#![feature(new_range_api)]
//! Mini Java 语言前端
//!
//! 提供 Mini Java 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;

use oak_java::{JavaLanguage, JavaRoot, JavaBuilder};
use oak_core::{source::SourceText, builder::Builder};
use nyar_vm::bytecode::format::NyarModule;
use chomsky_uir::{EGraph, IKun, ConstraintAnalysis};
use std::fmt::{Display, Formatter};
use std::error::Error;

#[derive(Debug)]
pub enum JavaError {
    Parse(String),
    Codegen(String),
    Other(String),
}

impl Display for JavaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            JavaError::Parse(msg) => write!(f, "Java parse error: {}", msg),
            JavaError::Codegen(msg) => write!(f, "Java codegen error: {}", msg),
            JavaError::Other(msg) => write!(f, "Java error: {}", msg),
        }
    }
}

impl Error for JavaError {}

impl From<String> for JavaError {
    fn from(s: String) -> Self {
        JavaError::Other(s)
    }
}

impl From<&str> for JavaError {
    fn from(s: &str) -> Self {
        JavaError::Other(s.to_string())
    }
}

pub type JavaResult<T> = Result<T, JavaError>;

/// Mini Java 前端
pub struct MiniJavaFrontend {
    language: JavaLanguage,
    builder: JavaBuilder,
}

impl MiniJavaFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let language = JavaLanguage::default();
        let builder = JavaBuilder::new(language.clone());
        Self { language, builder }
    }

    /// 解析 Java 源代码
    pub fn parse(&self, source: &str) -> JavaResult<JavaRoot> {
        let mut session = oak_core::parser::ParseSession::<JavaLanguage>::default();
        let source_text = SourceText::new(source);
        let output = self.builder.build(&source_text, &[], &mut session);
        match output.result {
            Ok(root) => Ok(root),
            Err(e) => Err(JavaError::Parse(format!("{:?}", e))),
        }
    }

    /// 编译到 Chomsky UIR (EGraph)
    pub fn compile_to_uir(&self, source: &str) -> JavaResult<EGraph<IKun, ConstraintAnalysis>> {
        let ast = self.parse(source)?;
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let translator = codegen::NyarTranslator::new();
        translator.translate_to_graph(&ast, &mut egraph)?;
        Ok(egraph)
    }

    /// 编译到 Nyar 字节码
    pub fn compile_to_nyar(&self, source: &str) -> JavaResult<NyarModule> {
        let ast = self.parse(source)?;
        let translator = codegen::NyarTranslator::new();
        translator.translate(&ast)
    }

    /// 从源代码生成后端产物
    pub fn generate_from_source(&self, source: &str) -> JavaResult<chomsky_extract::BackendArtifact> {
        let module = self.compile_to_nyar(source)?;
        let data = module.encode();
        Ok(chomsky_extract::BackendArtifact::Binary(data))
    }
}

impl Default for MiniJavaFrontend {
    fn default() -> Self {
        Self::new()
    }
}
