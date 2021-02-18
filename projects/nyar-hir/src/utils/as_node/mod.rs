use nyar_error::Span;

use crate::{
    ast::{DecimalLiteral, IntegerLiteral},
    ASTKind, ASTNode,
};

macro_rules! turn_node {
    ($($kind:ident => $node:ident),*) => {
        $(
            impl $kind {
                pub fn as_node(self, span: Span) -> ASTNode {
                    ASTNode { kind: ASTKind::$node(box self), span }
                }
            }
        )*
    };
}

turn_node! {
    IntegerLiteral => Integer,
    DecimalLiteral => Decimal
}
