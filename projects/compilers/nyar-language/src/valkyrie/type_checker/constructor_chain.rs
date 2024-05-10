//! 构造器链与 `super` 调用检查。

use crate::types::{Identifier, SourceSpan, hir::ValkyrieType as HirType};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstructorChainErrorKind {
    MissingSuperCall { class_name: Identifier, parent: Identifier },
    SuperCallArgumentMismatch { class_name: Identifier, parent: Identifier, expected: usize, got: usize },
    InvalidSuperCallOrder { class_name: Identifier, expected: Vec<Identifier>, actual: Vec<Identifier> },
    DuplicateSuperCall { class_name: Identifier, parent: Identifier },
    InvalidSuperCallMethod { class_name: Identifier, method: Identifier },
    SuperCallArgumentTypeMismatch { class_name: Identifier, parent: Identifier, index: usize, expected: HirType, got: HirType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorChainError {
    pub kind: ConstructorChainErrorKind,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl ConstructorChainError {
    pub fn missing_super_call(class_name: Identifier, parent: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: ConstructorChainErrorKind::MissingSuperCall { class_name: class_name.clone(), parent: parent.clone() },
            message: format!("{} is missing required super call to {}", class_name, parent),
            span,
        }
    }

    pub fn super_call_argument_mismatch(
        class_name: Identifier,
        parent: Identifier,
        expected: usize,
        got: usize,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            kind: ConstructorChainErrorKind::SuperCallArgumentMismatch {
                class_name: class_name.clone(),
                parent: parent.clone(),
                expected,
                got,
            },
            message: format!("Expected {} arguments for {} super call in {}, got {}", expected, parent, class_name, got),
            span,
        }
    }

    pub fn invalid_super_call_order(
        class_name: Identifier,
        expected: Vec<Identifier>,
        actual: Vec<Identifier>,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            kind: ConstructorChainErrorKind::InvalidSuperCallOrder {
                class_name: class_name.clone(),
                expected: expected.clone(),
                actual: actual.clone(),
            },
            message: format!("Invalid MRO super call order in {}", class_name),
            span,
        }
    }

    pub fn duplicate_super_call(class_name: Identifier, parent: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: ConstructorChainErrorKind::DuplicateSuperCall { class_name: class_name.clone(), parent: parent.clone() },
            message: format!("Duplicate super call to {} in {}", parent, class_name),
            span,
        }
    }

    pub fn invalid_super_call_method(class_name: Identifier, method: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: ConstructorChainErrorKind::InvalidSuperCallMethod { class_name: class_name.clone(), method: method.clone() },
            message: format!("{} may only call super from initiate, not {}", class_name, method),
            span,
        }
    }

    pub fn super_call_argument_type_mismatch(
        class_name: Identifier,
        parent: Identifier,
        index: usize,
        expected: HirType,
        got: HirType,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            kind: ConstructorChainErrorKind::SuperCallArgumentTypeMismatch {
                class_name: class_name.clone(),
                parent: parent.clone(),
                index,
                expected: expected.clone(),
                got: got.clone(),
            },
            message: format!(
                "super call argument type mismatch in {} -> {} at {}: expected {:?}, got {:?}",
                class_name, parent, index, expected, got
            ),
            span,
        }
    }
}

impl fmt::Display for ConstructorChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ConstructorChainError {}

#[derive(Debug, Default)]
pub struct ConstructorChainChecker {
    errors: Vec<ConstructorChainError>,
}

impl ConstructorChainChecker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn errors(&self) -> &[ConstructorChainError] {
        &self.errors
    }
}
