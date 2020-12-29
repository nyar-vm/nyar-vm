use super::*;
use std::{collections::BTreeSet, iter::FromIterator};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct BinaryExpression {
    pub o: Operator,
    pub lhs: ASTNode,
    pub rhs: ASTNode,
}

impl BinaryExpression {
    pub fn is_shortcut(&self) -> bool {
        let set = BTreeSet::from_iter(["&&", "||"].iter());
        match self.o {
            Operator::Infix { op, .. } => set.contains(&op),
            _ => false,
        }
    }
    pub fn as_function_call(&self) -> ASTKind {
        todo!()
    }
}
