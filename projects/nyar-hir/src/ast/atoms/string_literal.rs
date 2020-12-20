use super::*;

///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringLiteral {
    pub handler: String,
    pub literal: String,
}
