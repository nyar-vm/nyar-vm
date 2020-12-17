use super::*;

#[derive(Clone, Archive, Deserialize, Serialize)]
pub struct LetBind {
    pub name: String,
}