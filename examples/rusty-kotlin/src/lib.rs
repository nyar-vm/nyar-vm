#![feature(new_range_api)]
//! Mini Kotlin 语言前端
//!
//! 提供 Mini Kotlin 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;
pub mod errors;
pub mod row_type;
pub mod tagless;
pub mod visitor;

use nyar_types::{IKunTree, NyarError, NyarFrontend};
use oak_core::{builder::Builder, source::SourceText};
use oak_kotlin::{KotlinBuilder, KotlinLanguage, KotlinRoot};

/// Rusty Kotlin 前端
pub struct RustyKotlinFrontend {
    language: KotlinLanguage,
    builder: KotlinBuilder<'static>,
}

impl Default for RustyKotlinFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyKotlinFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let language = Box::leak(Box::new(KotlinLanguage::default()));
        let builder = KotlinBuilder::new(language);
        Self {
            language: language.clone(),
            builder,
        }
    }
}

impl NyarFrontend for RustyKotlinFrontend {
    type Language = KotlinLanguage;

    fn parse(&self, source: &str) -> Result<KotlinRoot, NyarError> {
        let mut session = oak_core::parser::ParseSession::<KotlinLanguage>::default();
        let source_text = SourceText::new(source);
        let output = self.builder.build(&source_text, &[], &mut session);
        if let Ok(root) = &output.result {
            println!("Parsed {} declarations", root.declarations.len());
        }
        output
            .result
            .map_err(|_| NyarError::Compile())
    }

    fn lower(&self, ast: &KotlinRoot) -> Result<IKunTree, NyarError> {
        let translator = codegen::NyarTranslator::new();
        let tree = translator.translate_to_tree(ast)?;
        println!("Lowered to tree: {:?}", tree);
        Ok(tree)
    }
}
