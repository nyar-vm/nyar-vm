//! Rusty Mojo 语言前端
//!
//! 这个库提供了 Rusty Mojo 语言的词法分析、语法分析和 Gaia 翻译功能。

pub mod codegen;

use nyar_types::{NyarContext, NyarError, NyarFrontend, Id, Vfs};
use oak_core::{Language, TokenType, ElementType, UniversalTokenRole, UniversalElementRole};
use chomsky_uir::ConstraintAnalysis;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MojoLanguage;

impl Language for MojoLanguage {
    const NAME: &'static str = "mojo";
    type TokenType = MojoSyntaxKind;
    type ElementType = MojoSyntaxKind;
    type TypedRoot = ();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MojoSyntaxKind {
    EndOfStream,
}

impl TokenType for MojoSyntaxKind {
    const END_OF_STREAM: Self = MojoSyntaxKind::EndOfStream;
    type Role = UniversalTokenRole;
    fn role(&self) -> Self::Role { UniversalTokenRole::None }
}

impl ElementType for MojoSyntaxKind {
    type Role = UniversalElementRole;
    fn role(&self) -> Self::Role { UniversalElementRole::None }
}

/// Rusty Mojo 前端
pub struct RustyMojoFrontend {
    language: MojoLanguage,
}

impl Default for RustyMojoFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyMojoFrontend {
    /// 创建新的前端实例
    pub fn new() -> Self {
        Self {
            language: MojoLanguage::default(),
        }
    }
}

impl NyarFrontend<ConstraintAnalysis> for RustyMojoFrontend {
    type Language = MojoLanguage;

    fn parse(&self, _source: &str) -> Result<(), NyarError> {
        Err(NyarError::Parse("Mojo parser not yet implemented".to_string()))
    }

    fn lower_unified<V: Vfs>(&self, _ast: &(), ctx: &mut NyarContext<V, ConstraintAnalysis>) -> Id {
        ctx.egraph.add(chomsky_uir::IKun::Seq(vec![]))
    }
}
