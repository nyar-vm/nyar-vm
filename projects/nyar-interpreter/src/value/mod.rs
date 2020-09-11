pub mod class;
pub mod error;
pub mod function;
pub mod utils;
pub mod variable;

mod format;
mod from_native;

use std::{
    any::Any,
    borrow::Cow,
    collections::VecDeque,
    fmt::{self, Debug, Display, Formatter},
};

use bigdecimal::BigDecimal;
use indexmap::IndexMap;
use num::BigInt;

use class::Class;
use function::FunctionInstance;

#[derive(Clone, Eq, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(Box<BigInt>),
    Decimal(Box<BigDecimal>),
    String(Box<String>),
    List(Box<VecDeque<Self>>),
    Object(Box<IndexMap<String, Self>>),
    Function(Box<FunctionInstance>),
    CustomClass(Box<dyn Class>),
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
    assert_eq!(std::mem::size_of::<Value>(), 24);
}
