use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct LetBind {
    pub name: String,
    pub body: ASTNode,
}
