pub mod class;
pub mod error;
pub mod function;
pub mod utils;
pub mod variable;

mod format;
mod from_native;

use std::{
    fmt::{self, Debug, Display, Formatter},
};

use crate::utils::OrderedMap;
use bigdecimal::BigDecimal;
use num::{BigInt, BigUint};

use class::Class;
use function::FunctionInstance;


#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    Character(char),
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
    Decimal32([u8; 4]),
    Decimal64([u8; 8]),

    Integer(Box<BigInt>),
    UnsignedInteger(BigUint),
    Decimal(Box<BigDecimal>),

    String(String),
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
    use std::borrow::Cow;
    use std::collections::VecDeque;

    assert_eq!(std::mem::size_of::<BigUint>(), 24);
    assert_eq!(std::mem::size_of::<BigInt>(), 32);

    assert_eq!(std::mem::size_of::<String>(), 24);
    assert_eq!(std::mem::size_of::<Cow<str>>(), 32);

    assert_eq!(std::mem::size_of::<Vec<Value>>(), 24);
    assert_eq!(std::mem::size_of::<VecDeque<Value>>(), 32);

    assert_eq!(std::mem::size_of::<Value>(), 32);
}
