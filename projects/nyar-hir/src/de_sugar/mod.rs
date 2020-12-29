use crate::{
    ast::{BinaryExpression, LetBind},
    ASTKind, ASTNode,
};

impl BinaryExpression {
    pub fn de_sugar(&self) -> ASTKind {
        todo!()
    }
}

impl LetBind {
    pub fn de_sugar(&self) -> ASTKind {
        todo!()
    }
}
