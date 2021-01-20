use super::*;


#[derive(Clone, Debug, Serialize, Deserialize)]

pub struct BinaryExpression {
    pub o: Operator,
    pub lhs: ASTNode,
    pub rhs: ASTNode,
}

impl BinaryExpression {
    pub fn is_shortcut(&self) -> bool {
        self.o.is_shortcut()
    }
    pub fn as_function_call(&self) -> ASTKind {
        todo!()
    }
}
