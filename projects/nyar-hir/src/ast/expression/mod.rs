use super::*;

pub(crate) mod apply;
pub(crate) mod arguments;
pub(crate) mod dot;
pub(crate) mod infix;

pub struct Expression {
    pub base: ASTNode,
    pub eos: bool,
}

pub struct ChainBuilder {
    base: ASTNode,
}

impl ChainBuilder {
    pub fn new(base: ASTNode) -> Self {
        Self { base }
    }
    pub fn as_node(self) -> ASTNode {
        self.base
    }
}
