mod byte_literal;
mod comment_literal;
mod dict_literal;
mod number_literal;
mod operator;
mod string_literal;
mod symbol;

use std::sync::Arc;
pub use self::{
    byte_literal::ByteLiteral, comment_literal::CommentLiteral, dict_literal::KVPair, number_literal::NumberLiteral,
    operator::Operator, string_literal::StringLiteral, symbol::Symbol,
};

use super::*;

#[derive(Clone, Archive, Deserialize, Serialize)]
pub enum ASTAtom {
    Boolean(bool),
    String(Arc<String>),
}