#![feature(new_range_api)]
//! Mini Java 语言前端
//!
//! 提供 Mini Java 的词法分析、语法分析和 Nyar 翻译功能。

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{builder::Builder, source::SourceText};
use oak_java::{JavaBuilder, JavaLanguage, JavaRoot};

pub mod codegen;
pub mod row_type;
pub mod tagless;
pub mod visitor;

/// Mini Java 前端
pub struct MiniJavaFrontend<'a> {
    language: JavaLanguage,
    builder: JavaBuilder<'a>,
}

impl<'a> Default for MiniJavaFrontend<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> MiniJavaFrontend<'a> {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let language = JavaLanguage::default();
        // We need a stable reference to language for JavaBuilder.
        // Since MiniJavaFrontend owns language, we can use unsafe or just leak it for now
        // if we want to avoid complex lifetime management in a quick fix.
        // But better is to make it hold a Box and reference that if possible, 
        // or just accept that the builder will have a lifetime tied to the language.
        
        // Actually, JavaBuilder in oak-java seems to take &'config JavaLanguage.
        // Let's use Box::leak for the language to get a 'static reference for simplicity in this example.
        let language_ref = Box::leak(Box::new(language));
        Self {
            language: JavaLanguage::default(), // This is redundant but kept for struct shape if needed
            builder: JavaBuilder::new(language_ref),
        }
    }
}

impl<'a> NyarFrontend for MiniJavaFrontend<'a> {
    type Language = JavaLanguage;

    /// 解析 Java 源代码
    fn parse(&self, source: &str) -> Result<JavaRoot, NyarError> {
        let mut session = oak_core::parser::ParseSession::<JavaLanguage>::default();
        let source_text = SourceText::new(source);
        let output = self.builder.build(&source_text, &[], &mut session);
        output
            .result
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))
    }

    /// 编译到 Chomsky UIR (IKunTree)
    fn lower(&self, ast: &JavaRoot) -> Result<IKunTree, NyarError> {
        let translator = codegen::NyarTranslator::new();
        translator.translate_to_tree(ast)
    }
}
