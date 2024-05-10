//! 访问级别与成员可见性检查。

use super::path_helpers::display_path;
use crate::types::{
    Identifier, NamePath, SourceSpan,
    hir::{AccessLevel, HirStruct},
};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessContext {
    pub current_module: NamePath,
    pub current_class: Option<Identifier>,
    pub current_method: Option<Identifier>,
}

impl AccessContext {
    pub fn new(current_module: NamePath) -> Self {
        Self { current_module, current_class: None, current_method: None }
    }

    pub fn in_class(current_module: NamePath, current_class: Identifier) -> Self {
        Self { current_module, current_class: Some(current_class), current_method: None }
    }

    pub fn in_method(current_module: NamePath, current_class: Identifier, current_method: Identifier) -> Self {
        Self { current_module, current_class: Some(current_class), current_method: Some(current_method) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessControlErrorKind {
    PrivateMemberAccess { owner: Identifier, member: Identifier, accessor: Option<Identifier> },
    ProtectedMemberAccess { owner: Identifier, member: Identifier, accessor: Identifier },
    InternalMemberAccess { member: Identifier, declared_module: NamePath, current_module: NamePath },
    ReadonlyFieldWrite { owner: Identifier, field: Identifier },
    PrivateConstructorInstantiation { class_name: Identifier },
    VisibilityReduction { class_name: Identifier, member: Identifier, from: AccessLevel, to: AccessLevel },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessControlError {
    pub kind: AccessControlErrorKind,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl AccessControlError {
    pub fn private_member_access(owner: Identifier, member: Identifier, accessor: Option<Identifier>, span: Option<SourceSpan>) -> Self {
        let accessor_text = accessor.as_ref().map(|name| format!(" by {}", name)).unwrap_or_default();
        Self {
            kind: AccessControlErrorKind::PrivateMemberAccess { owner: owner.clone(), member: member.clone(), accessor },
            message: format!("private member access: {}.{}{}", owner, member, accessor_text),
            span,
        }
    }

    pub fn protected_member_access(owner: Identifier, member: Identifier, accessor: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: AccessControlErrorKind::ProtectedMemberAccess { owner: owner.clone(), member: member.clone(), accessor: accessor.clone() },
            message: format!("protected member access denied: {}.{} from {}", owner, member, accessor),
            span,
        }
    }

    pub fn internal_member_access(member: Identifier, declared_module: NamePath, current_module: NamePath, span: Option<SourceSpan>) -> Self {
        Self {
            kind: AccessControlErrorKind::InternalMemberAccess {
                member: member.clone(),
                declared_module: declared_module.clone(),
                current_module: current_module.clone(),
            },
            message: format!(
                "internal member access denied: {} declared in {} from {}",
                member,
                display_path(&declared_module),
                display_path(&current_module)
            ),
            span,
        }
    }

    pub fn readonly_field_write(owner: Identifier, field: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: AccessControlErrorKind::ReadonlyFieldWrite { owner: owner.clone(), field: field.clone() },
            message: format!("readonly field write denied: {}.{}", owner, field),
            span,
        }
    }

    pub fn private_constructor_instantiation(class_name: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: AccessControlErrorKind::PrivateConstructorInstantiation { class_name: class_name.clone() },
            message: format!("private constructor instantiation denied: {} requires a factory method", class_name),
            span,
        }
    }

    pub fn visibility_reduction(
        class_name: Identifier,
        member: Identifier,
        from: AccessLevel,
        to: AccessLevel,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            kind: AccessControlErrorKind::VisibilityReduction { class_name: class_name.clone(), member: member.clone(), from, to },
            message: format!("reduced visibility is not allowed: {}.{} from {} to {}", class_name, member, from.as_str(), to.as_str()),
            span,
        }
    }
}

impl fmt::Display for AccessControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AccessControlError {}

#[derive(Debug, Default)]
pub struct AccessControlChecker {
    errors: Vec<AccessControlError>,
    classes: BTreeMap<Identifier, HirStruct>,
    inheritance_map: BTreeMap<Identifier, Vec<Identifier>>,
}

impl AccessControlChecker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn errors(&self) -> &[AccessControlError] {
        &self.errors
    }

    pub fn classes(&self) -> &BTreeMap<Identifier, HirStruct> {
        &self.classes
    }

    pub fn inheritance_map(&self) -> &BTreeMap<Identifier, Vec<Identifier>> {
        &self.inheritance_map
    }
}
