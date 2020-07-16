use std::{borrow::Cow, collections::VecDeque};

use bigdecimal::BigDecimal;
use indexmap::IndexMap;
use num::BigInt;

use crate::function::FunctionInstance;
use std::fmt::{self, Display, Formatter};

mod from_native;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(Box<BigInt>),
    Decimal(Box<BigDecimal>),
    String(Box<String>),
    Array(Box<VecDeque<Self>>),
    Object(Box<IndexMap<String, Self>>),
    Function(Box<FunctionInstance>),
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Boolean(v) => write!(f, "{}", v),
            Value::Integer(v) => write!(f, "{}", v),
            Value::Decimal(v) => write!(f, "{}", v),
            Value::String(v) => write!(f, "{}", v),
            Value::Array(v) => write!(f, "{:?}", v),
            Value::Object(v) => write!(f, "{:?}", v),
            Value::Function(v) => write!(f, "{:?}", v),
        }
    }
}

#[allow(non_upper_case_globals)]
impl Value {
    pub const True: Self = Value::Boolean(true);
    pub const False: Self = Value::Boolean(false);
}

#[test]
fn check_size() {
    assert_eq!(std::mem::size_of::<String>(), 24);
    assert_eq!(std::mem::size_of::<Cow<str>>(), 32);
    assert_eq!(std::mem::size_of::<VecDeque<Value>>(), 32);
    assert_eq!(std::mem::size_of::<IndexMap<String, Value>>(), 72);
    assert_eq!(std::mem::size_of::<Value>(), 16);
}
