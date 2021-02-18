use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegerLiteral {
    pub handler: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecimalLiteral {
    pub handler: String,
    pub value: String,
}

impl Default for IntegerLiteral {
    fn default() -> Self {
        Self { handler: "".to_string(), value: "0".to_string() }
    }
}

impl Default for DecimalLiteral {
    fn default() -> Self {
        Self { handler: "".to_string(), value: "0".to_string() }
    }
}

impl Display for IntegerLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.handler)
    }
}

impl Display for DecimalLiteral {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.value, self.handler)
    }
}

impl IntegerLiteral {
    pub fn new<S>(number: &str) -> Self
    where
        S: Into<String>,
    {
        Self { handler: String::new(), value: number.into() }
    }
    pub fn with_handler<S>(number: &str, handler: &str) -> Self
    where
        S: Into<String>,
    {
        Self { handler: handler.to_string(), value: number.into() }
    }

    fn remove_underscore() {}
}
