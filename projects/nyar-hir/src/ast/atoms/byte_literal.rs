use super::*;

/// - `Number`: raw number represent
#[derive(Debug, Clone)]
pub struct ByteLiteral {
    pub handler: Option<String>,
    pub value: String,
}
