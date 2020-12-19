use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct LetBind {
    pub name: String,
    pub body: ASTNode,
}
