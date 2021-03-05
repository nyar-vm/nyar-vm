use super::*;

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
}
