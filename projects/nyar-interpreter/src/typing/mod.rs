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

impl Display for Typing {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match (&self.typing, &self.effect) {
            (Some(t), Some(e)) => write!(f, "-> {} / {{{:?}}}", t, e),
            (Some(t), None) => write!(f, "-> {}", t),
            (None, Some(e)) => write!(f, "-> / {{{:?}}}", e),
            (None, None) => Ok(()),
        }
    }
}

impl Display for TypingExpression {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Null => {
                write!(f, "null")
            }
            Self::Boolean => {
                write!(f, "bool")
            }
            Self::Literal(value) => {
                write!(f, "{}", value)
            }
            Self::Union(terms) => {
                assert!(!terms.is_empty());
                for (index, term) in terms.iter().enumerate() {
                    write!(f, "{}", term)?;
                    if index != terms.len() - 1 {
                        write!(f, " | ")?;
                    }
                }
                Ok(())
            }
            Self::Tuple(terms) => {
                write!(f, "(")?;
                for (index, term) in terms.iter().enumerate() {
                    write!(f, "{}", term)?;
                    if index != terms.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ")")
            }
        }
    }
}
