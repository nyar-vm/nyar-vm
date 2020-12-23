use super::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct LetBind {
    pub name: String,
    pub body: ASTNode,
}
