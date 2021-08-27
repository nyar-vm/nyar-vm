use super::*;

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

impl Default for DecimalLiteral {
    fn default() -> Self {
        Self { handler: "".to_string(), value: BigDecimal::zero() }
    }
}

impl FromStr for DecimalLiteral {
    type Err = NyarError3;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { handler: String::new(), value: BigDecimal::from_str(s)? })
    }
}

impl DecimalLiteral {
    pub fn with_handler(self, handler: &str) -> Self {
        Self { handler: handler.to_string(), value: self.value }
    }
    //noinspection RsSelfConvention
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::Decimal(box self), span }
    }
}
