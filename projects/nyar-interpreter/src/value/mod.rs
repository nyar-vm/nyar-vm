use std::{borrow::Cow, collections::VecDeque};
use std::{
    any::Any,
    fmt::{self, Debug, Display, Formatter},
};

use bigdecimal::BigDecimal;
use indexmap::IndexMap;
use num::BigInt;

use crate::{class::Class, function::FunctionInstance};

mod from_native;
pub mod utils;

#[derive(Clone, Eq, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(Box<BigInt>),
    Decimal(Box<BigDecimal>),
    String(Box<String>),
    Array(Box<VecDeque<Self>>),
    Object(Box<IndexMap<String, Self>>),
    Function(Box<FunctionInstance>),
    CustomClass(Box<dyn Class>),
}

impl Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.debug_tuple("Null").finish(),
            Value::Boolean(v) => f.debug_tuple("Boolean").field(v).finish(),
            Value::Integer(v) => f.debug_tuple("Integer").field(v).finish(),
            Value::Decimal(v) => f.debug_tuple("Decimal").field(v).finish(),
            Value::String(v) => f.debug_tuple("String").field(v).finish(),
            Value::Array(v) => f.debug_tuple("Array").field(v).finish(),
            Value::Object(v) => f.debug_tuple("Object").field(v).finish(),
            Value::Function(v) => f.debug_tuple("Function").field(v).finish(),
            Value::CustomClass(_) => f.debug_tuple("Class").finish(),
        }
    }
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
            Value::CustomClass(_) => write!(f, "Class"),
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
