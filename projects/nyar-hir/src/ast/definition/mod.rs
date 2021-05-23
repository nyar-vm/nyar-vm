use super::*;

pub(crate) mod function;

#[derive(Clone, Debug, Serialize, Deserialize)]

pub struct LetBind {
    pub name: String,
    pub body: ASTNode,
}
