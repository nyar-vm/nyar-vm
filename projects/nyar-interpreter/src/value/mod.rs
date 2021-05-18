pub mod class;
pub mod error;
mod format;
mod from_native;
pub mod function;
mod lists;
pub mod maybe;
pub mod numbers;
pub mod symbol;
pub mod utils;
pub mod variable;

pub use self::class::NyarClass;
use crate::Result;
use std::{
    collections::{BTreeMap, VecDeque},
    fmt::{self, Debug, Display, Formatter},
};

use self::function::FunctionInstance;
use crate::utils::OrderedMap;
use bigdecimal::BigDecimal;
use num::{BigInt, BigUint};

// use crate::value::maybe::{Maybe, Validation};
use crate::value::function::FunctionPrototype;
use shredder::{
    marker::{GcDrop, GcSafe},
    plumbing::check_gc_drop,
    Gc, Scan, Scanner,
};
use std::sync::{Arc, RwLock};

pub type SharedValue = Gc<RwLock<Value>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Null,
    Unit,
    Boolean(bool),

    Integer(BigInt),
    Decimal(BigDecimal),

    Character(char),
    String(String),

    List(Vec<Self>),
    Vector(Vec<Self>),
    Tuple(Vec<Self>),
    Suite(Vec<Self>),
    Object(OrderedMap<String, Self>),
    FunctionDefinition(FunctionPrototype),
    Function(FunctionInstance),
    // CustomClass(Box<dyn Class>),
}

#[allow(non_upper_case_globals)]
impl Value {
    pub const True: Self = Value::Boolean(true);
    pub const False: Self = Value::Boolean(false);
}

#[test]
fn check_size() {
    use std::{borrow::Cow, collections::VecDeque, rc::Rc, sync::Arc};

    assert_eq!(std::mem::size_of::<Box<Value>>(), 8);
    assert_eq!(std::mem::size_of::<Vec<Value>>(), 24);
    assert_eq!(std::mem::size_of::<Gc<Value>>(), 32);
    assert_eq!(std::mem::size_of::<Rc<Value>>(), 8);
    assert_eq!(std::mem::size_of::<Arc<Value>>(), 8);

    assert_eq!(std::mem::size_of::<BigUint>(), 24);
    assert_eq!(std::mem::size_of::<BigInt>(), 32);

    assert_eq!(std::mem::size_of::<String>(), 24);
    assert_eq!(std::mem::size_of::<Cow<str>>(), 32);
    assert_eq!(std::mem::size_of::<[u8; 31]>(), 31);

    assert_eq!(std::mem::size_of::<VecDeque<Value>>(), 32);

    assert_eq!(std::mem::size_of::<Value>(), 32);
}
