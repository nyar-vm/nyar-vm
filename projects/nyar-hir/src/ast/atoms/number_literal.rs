use super::*;

#[derive(Debug, Clone)]
pub struct NumberLiteral {
    handler: Option<StringRange>,
    value: StringRange,
}
