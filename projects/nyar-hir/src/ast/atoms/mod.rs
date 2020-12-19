mod byte_literal;
mod comment_literal;
mod dict_literal;
mod number_literal;
mod operator;
mod string_literal;
mod symbol;

pub use self::{
    byte_literal::ByteLiteral, comment_literal::CommentLiteral, dict_literal::KVPair, number_literal::NumberLiteral,
    operator::Operator, string_literal::StringLiteral, symbol::Symbol,
};
use num::BigInt;
use std::sync::Arc;

use super::*;

/// Literals that without any effects
#[derive(Clone, Serialize, Deserialize)]
pub enum ASTAtom {
    Boolean(bool),
    Integer(BigInt),
    String(String),
    Symbol(String),
}
