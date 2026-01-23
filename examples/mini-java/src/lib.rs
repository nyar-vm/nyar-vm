#![feature(new_range_api)]
//! Mini Java 语言前端
//!
//! 提供 Mini Java 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;

use oak_java::{JavaLanguage, JavaRoot, JavaBuilder};
use oak_core::{source::SourceText, builder::Builder};
use nyar_vm::bytecode::format::NyarModule;
use chomsky_uir::{EGraph, IKun, ConstraintAnalysis};
use anyhow::Result;

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
    pub fn parse(&self, source: &str) -> Result<JavaRoot> {
        let mut session = oak_core::parser::ParseSession::<JavaLanguage>::default();
        let source_text = SourceText::new(source);
        let output = self.builder.build(&source_text, &[], &mut session);
        match output.result {
            Ok(root) => Ok(root),
            Err(e) => Err(anyhow::anyhow!("Parse error: {:?}", e)),
        }
    }

    /// 编译到 Chomsky UIR (EGraph)
    pub fn compile_to_uir(&self, source: &str) -> Result<EGraph<IKun, ConstraintAnalysis>> {
        let ast = self.parse(source)?;
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let translator = codegen::NyarTranslator::new();
        translator.translate_to_graph(&ast, &mut egraph)?;
        Ok(egraph)
    }

    /// 编译到 Nyar 字节码
    pub fn compile_to_nyar(&self, source: &str) -> Result<NyarModule> {
        let ast = self.parse(source)?;
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let translator = codegen::NyarTranslator::new();
        translator.translate_to_graph(&ast, &mut egraph)?;
        translator.translate(&ast)
    }
}

impl Default for MiniJavaFrontend {
    fn default() -> Self {
        Self::new()
    }
}
