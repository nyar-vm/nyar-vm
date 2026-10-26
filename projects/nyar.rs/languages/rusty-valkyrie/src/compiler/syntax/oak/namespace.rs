//! Oak `namespace` AST helpers (declaration-only vs braced body).

use oak_valkyrie::ast::{NamespaceDeclaration, StatementNode};

/// `namespace foo;` — switches the active namespace without introducing items.
pub(crate) fn namespace_is_declaration_only(namespace: &NamespaceDeclaration) -> bool {
    namespace.items.is_empty()
}

/// Items inside a braced `namespace foo { ... }`.
pub(crate) fn namespace_items(namespace: &NamespaceDeclaration) -> &[StatementNode] {
    &namespace.items
}

pub(crate) fn namespace_items_mut(namespace: &mut NamespaceDeclaration) -> &mut Vec<StatementNode> {
    &mut namespace.items
}
