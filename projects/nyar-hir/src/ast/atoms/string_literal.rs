use super::*;

///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringLiteral {
    pub handler: String,
    pub literal: String,
}

impl Display for StringLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.handler, self.literal)
    }
}
