use std::{
    fmt::{self, Display, Formatter},
};

use crate::{value::function::EffectHandler, Value};

mod display;
pub mod effects;
mod refine;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Typing {
    typing: Option<TypingExpression>,
    effect: Option<EffectExpression>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TypingExpression {
    Null,
    Boolean,
    Literal(Box<Value>),
    Union(Vec<TypingExpression>),
    Tuple(Vec<TypingExpression>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EffectExpression {
    inner: Vec<EffectHandler>,
}

impl TypingExpression {
    pub fn boolean(v: bool) -> Self {
        match v {
            true => TypingExpression::Literal(box Value::True),
            false => TypingExpression::Literal(box Value::False),
        }
    }
}
