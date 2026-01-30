#![feature(new_range_api)]
//! Mini Kotlin 语言前端
//!
//! 提供 Mini Kotlin 的词法分析、语法分析和 Nyar 翻译功能。

pub mod codegen;
pub mod visitor;
pub mod tagless;
pub mod row_type;
pub mod errors;

use oak_kotlin::{KotlinLanguage, KotlinRoot, KotlinBuilder};
use oak_core::{source::SourceText, builder::Builder};
use nyar_types::{NyarFrontend, NyarError, IKunTree};

/// Mini Kotlin 前端
pub struct MiniKotlinFrontend {
    language: KotlinLanguage,
    builder: KotlinBuilder<'static>,
}

impl Default for MiniKotlinFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl MiniKotlinFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        let language = Box::leak(Box::new(KotlinLanguage::default()));
        let builder = KotlinBuilder::new(language);
        Self { language: language.clone(), builder }
    }
}

impl NyarFrontend for MiniKotlinFrontend {
    type Language = KotlinLanguage;

    fn parse(&self, source: &str) -> Result<KotlinRoot, NyarError> {
        let mut session = oak_core::parser::ParseSession::<KotlinLanguage>::default();
        let source_text = SourceText::new(source);
        let output = self.builder.build(&source_text, &[], &mut session);
        if let Ok(root) = &output.result {
            println!("Parsed {} declarations", root.declarations.len());
        }
        output.result.map_err(|e| NyarError::Parse(format!("{:?}", e)))
    }

    fn lower(&self, ast: &KotlinRoot) -> Result<IKunTree, NyarError> {
        let translator = codegen::NyarTranslator::new();
        let tree = translator.translate_to_tree(ast)?;
        println!("Lowered to tree: {:?}", tree);
        Ok(tree)
    }
}
