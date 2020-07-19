mod display;
mod refine;

use crate::{function::EffectHandler, Value};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fmt::{self, Display, Formatter},
};

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
