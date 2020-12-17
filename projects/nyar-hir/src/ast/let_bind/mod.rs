use super::*;

#[derive(Clone, Deserialize, Serialize)]
pub struct LetBind {
    pub name: String,
    pub body: ASTNode,
}
