use super::*;

pub(crate) mod infix;

pub struct Expression {
    pub base: ASTNode,
    pub eos: bool,
}
