use super::*;
/// - `Number`: raw number represent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByteLiteral {
    pub handler: char,
    pub value: String,
}
