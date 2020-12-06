use super::*;

pub enum LetBindStatement {
    Simple(Box<LetBindSimple>),
    Block(Box<AssignBlock>),
}

pub struct LetBindSimple {
    lhs: ASTNode,
    typing: ASTNode,
    rhs: ASTNode,
    eos: bool,
}

pub struct AssignBlock {}
