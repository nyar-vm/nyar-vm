use super::*;

///
#[derive(Debug, Clone)]
pub struct StringLiteral {
   pub handler: Option<String>,
   pub value: Vec<ASTNode>,
}
