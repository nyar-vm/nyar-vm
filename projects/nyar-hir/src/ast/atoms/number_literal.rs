use nyar_error::{
    third_party::{num::Zero, BigDecimal, BigInt},
    NyarError,
};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegerLiteral {
    pub handler: String,
    pub value: BigInt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecimalLiteral {
    pub handler: String,
    pub value: BigDecimal,
}

/// - `Number`: raw number represent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByteLiteral {
    pub handler: char,
    pub value: String,
}

impl Default for IntegerLiteral {
    fn default() -> Self {
        Self { handler: "".to_string(), value: BigInt::zero() }
    }
}

impl Default for DecimalLiteral {
    fn default() -> Self {
        Self { handler: "".to_string(), value: BigDecimal::zero() }
    }
}

impl FromStr for IntegerLiteral {
    type Err = NyarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { handler: String::new(), value: BigInt::from_str(s)? })
    }
}

impl FromStr for DecimalLiteral {
    type Err = NyarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { handler: String::new(), value: BigDecimal::from_str(s)? })
    }
}

impl IntegerLiteral {
    pub fn with_handler(mut self, handler: &str) -> Self {
        Self { handler: handler.to_string(), value: self.value }
    }
}

impl DecimalLiteral {
    pub fn with_handler(mut self, handler: &str) -> Self {
        Self { handler: handler.to_string(), value: self.value }
    }
}
