use nyar_error::{
    third_party::{num::Zero, BigInt},
    NyarError3,
};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegerLiteral {
    pub handler: String,
    pub value: BigInt,
}

impl Default for IntegerLiteral {
    fn default() -> Self {
        Self { handler: "".to_string(), value: BigInt::zero() }
    }
}

impl FromStr for IntegerLiteral {
    type Err = NyarError3;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { handler: String::new(), value: BigInt::from_str(s)? })
    }
}

impl IntegerLiteral {
    pub fn with_handler(self, handler: &str) -> Self {
        Self { handler: handler.to_string(), value: self.value }
    }
    //noinspection RsSelfConvention
    pub fn as_node(self, span: Span) -> ASTNode {
        ASTNode { kind: ASTKind::Integer(box self), span }
    }
}
