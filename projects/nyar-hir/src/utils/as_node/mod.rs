use nyar_error::Span;

use crate::{
    ast::{DecimalLiteral, IntegerLiteral, KVPair, Symbol, TableExpression},
    ASTKind, ASTNode,
};

impl IntegerLiteral {
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::Integer(box self), span }
    }
}

impl DecimalLiteral {
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::Decimal(box self), span }
    }
}

impl KVPair {
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::PairExpression(box self), span }
    }
}

impl TableExpression {
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::TableExpression(box self), span }
    }
}

impl Symbol {
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::Symbol(box self), span }
    }
}
