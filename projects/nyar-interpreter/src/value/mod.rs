pub mod class;
pub mod error;
pub mod function;
pub mod numbers;
pub mod symbol;
pub mod utils;
pub mod variable;

mod format;
mod from_native;

pub use self::{class::NyarClass, numbers::FloatWrapper, symbol::Symbol};

use std::fmt::{self, Debug, Display, Formatter};

use self::function::FunctionInstance;
use crate::utils::OrderedMap;
use bigdecimal::BigDecimal;
use num::{BigInt, BigUint};

use shredder::{
    marker::{GcDrop, GcSafe},
    plumbing::check_gc_drop,
    Gc, Scan, Scanner,
};
use std::sync::RwLock;

pub type SharedValue = Gc<RwLock<Value>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),

    Integer8(i8),
    Integer16(i16),
    Integer32(i32),
    Integer64(i64),
    Integer128(i128),
    IntegerSized(isize),
    UnsignedInteger8(u8),
    UnsignedInteger16(u16),
    UnsignedInteger32(u32),
    UnsignedInteger64(u64),
    UnsignedInteger128(u128),
    UnsignedIntegerSized(usize),
    Decimal32(FloatWrapper<f32>),
    Decimal64(FloatWrapper<f64>),

    InlineInteger(i128),
    Integer(Box<BigInt>),
    UnsignedInteger(BigUint),
    Decimal(Box<BigDecimal>),

    Character(char),
    String(String),
    InlineString([u8; 31]),

    List(Vec<Self>),
    Suite(Vec<Self>),
    Object(Box<OrderedMap<String, Self>>),
    Function(Box<FunctionInstance>),
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
