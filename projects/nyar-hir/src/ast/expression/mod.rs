use super::*;

pub mod infix;

pub struct Expression {
    pub base: ASTNode,
    pub eos: bool,
}
