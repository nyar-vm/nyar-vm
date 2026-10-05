//! `AST -> HIR` 前的结构与语义预校验。

use crate::valkyrie::frontend::{
    ParseError, ValkyrieRoot,
    ast::{EnumsKind, StatementNode},
};
use std::collections::BTreeMap;

/// 校验 Oak `AST` 根节点是否满足进入 `HIR` lowering 的前提。
pub(crate) fn validate_ast_root(root: &ValkyrieRoot) -> Result<(), ParseError> {
    validate_duplicate_type_names(&root.items)
}

fn validate_duplicate_type_names(items: &[StatementNode]) -> Result<(), ParseError> {
    let mut seen: BTreeMap<String, &'static str> = BTreeMap::new();
    for item in items {
        match item {
            StatementNode::Namespace(namespace) if !namespace.items.is_empty() => {
                validate_duplicate_type_names(&namespace.items)?;
            }
            other => {
                if let Some((name, kind)) = type_declaration_identity(other) {
                    if let Some(prior) = seen.insert(name.clone(), kind) {
                        return Err(ParseError::invalid(format!("duplicate definition of `{name}`: {prior} and {kind}")));
                    }
                }
            }
        }
    }
    Ok(())
}

fn type_declaration_identity(item: &StatementNode) -> Option<(String, &'static str)> {
    match item {
        StatementNode::Class(item) => Some((item.name.name.clone(), "class")),
        StatementNode::Structure(item) => Some((item.name.name.clone(), "structure")),
        StatementNode::Union(item) => Some((item.name.name.clone(), "union")),
        StatementNode::Singleton(item) => Some((item.name.name.clone(), "singleton")),
        StatementNode::Flags(item) => Some((item.name.name.clone(), "flags")),
        StatementNode::Enums(item) => {
            let kind = match item.kind {
                EnumsKind::Unity => "unite",
                EnumsKind::Enums | EnumsKind::Enum => "enums",
            };
            Some((item.name.name.clone(), kind))
        }
        StatementNode::Variant(item) => Some((item.name.name.clone(), "unite")),
        StatementNode::Trait(item) => Some((item.name.name.clone(), "trait")),
        StatementNode::Widget(item) => Some((item.name.name.clone(), "widget")),
        StatementNode::TypeAlias(item) => Some((item.name.name.clone(), "type alias")),
        _ => None,
    }
}
