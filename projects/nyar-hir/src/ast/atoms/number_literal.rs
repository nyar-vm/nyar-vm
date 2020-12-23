use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct NumberLiteral {
    pub handler: String,
    pub value: String,
}

impl Display for NumberLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.handler)
    }
}

impl Default for NumberLiteral {
    fn default() -> Self {
        Self { handler: "".to_string(), value: "".to_string() }
    }
}
