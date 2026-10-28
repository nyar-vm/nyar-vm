//! Oak `namespace` AST helpers (declaration-only vs braced body).

use oak_valkyrie::ast::{NamespaceDeclaration, StatementNode};

/// `namespace foo;` — switches the active namespace without introducing items.
pub(crate) fn namespace_is_declaration_only(namespace: &NamespaceDeclaration) -> bool {
    namespace.body.as_ref().map(|b| b.items.is_empty()).unwrap_or(true)
}

/// Items inside a braced `namespace foo { ... }`.
pub(crate) fn namespace_items(namespace: &NamespaceDeclaration) -> &[StatementNode] {
    namespace.body.as_ref().map(|b| b.items.as_slice()).unwrap_or(&[])
}

pub(crate) fn namespace_items_mut(namespace: &mut NamespaceDeclaration) -> &mut Vec<StatementNode> {
    namespace.body.as_mut().map(|b| &mut b.items).unwrap_or_else(|| panic!("namespace body is None"))
}
