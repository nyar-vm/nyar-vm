use std::fmt::{self, Display};
use std::str::FromStr;
use bigdecimal::BigDecimal;
use gc::{Gc, GcCell};
use num::traits::FromPrimitive;
use num::BigInt;

use crate::value::function::Function;
use crate::value::object::ObjectData;

use super::number::Number as NyarNumber;

/// 受 Garbage Collector 管理的值
pub type Value = Gc<ValueData>;

/// Nyar 7 大原生核心类型:
/// - Boolean/String/Integer/Decimal/Function/Object
/// - 以及 Type 类型本身
#[derive(Trace, Finalize, Debug, Clone)]
pub enum ValueData {
    /// `Boolean` - 真值类型 `true` / `false`
    Boolean(bool),
    /// `String` - 字符串类型
    String(String),
    /// `Number` - 高精度整数类型
    Number(NyarNumber),
    /// `Object` - 对象模型
    Object(GcCell<ObjectData>),
    /// `Function` - 函数类型
    Function(Box<GcCell<Function>>),
    /// `Type` - 特殊类型/高阶类型
    Type(GcCell<ObjectData>), //TODO: TypeData
}

impl Display for ValueData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ValueData::Type(ref _v) => write!(f, "Type"),
            ValueData::Boolean(ref v) => write!(f, "{}", v),
            ValueData::String(ref v) => write!(f, "{}", v),
            ValueData::Number(ref v) => write!(f, "{}", v),
            ValueData::Object(ref _v) => write!(f, "Object"),
            ValueData::Function(ref _v) => write!(f, "Function"),
        }
    }
}

// region BasicTypeFrom
impl From<bool> for ValueData {
    fn from(b: bool) -> ValueData {
        ValueData::Boolean(b)
    }
}

impl From<&str> for ValueData {
    fn from(s: &str) -> ValueData {
        ValueData::String(s.to_string())
    }
}

impl From<String> for ValueData {
    fn from(s: String) -> ValueData {
        ValueData::String(s)
    }
}

macro_rules! wrap_type {
    ($T:ty, $F:ident) => {
        impl From<$T> for ValueData {
            fn from(n: $T) -> ValueData {
                let i = NyarNumber::$F(n);
                ValueData::Number(i)
            }
        }
    };
}

wrap_type!(BigInt, Integer);
wrap_type!(i8, Integer8);
wrap_type!(i16, Integer16);
wrap_type!(i32, Integer32);
wrap_type!(i64, Integer64);
wrap_type!(i128, Integer128);

wrap_type!(u8, Unsigned8);
wrap_type!(u16, Unsigned16);
wrap_type!(u32, Unsigned32);
wrap_type!(u64, Unsigned64);
wrap_type!(u128, Unsigned128);

wrap_type!(BigDecimal, Decimal);
wrap_type!(f32, Float32);
wrap_type!(f64, Float64);
// endregion
// region NumberTypeFrom
pub trait NewBoolean<T> {
    fn new_bool(t: T) -> ValueData;
}

pub trait NewNumber<T> {
    fn new_int(t: T) -> ValueData;
    fn new_dec(t: T) -> ValueData;
}

pub trait NewDecimal<T, U> {
    fn new_dec_scale(t: T, s: U) -> ValueData;
}

pub trait NewString<T> {
    fn new_str(t: T) -> ValueData;
}

impl NewBoolean<bool> for ValueData {
    fn new_bool(b: bool) -> ValueData {
        ValueData::Boolean(b)
    }
}

impl NewString<&str> for ValueData {
    fn new_str(s: &str) -> ValueData {
        ValueData::String(s.to_string())
    }
}


impl<T> NewDecimal<&str, T> for ValueData
where
    i64: From<T>,
{
    fn new_dec_scale(s: &str, u: T) -> ValueData {
        let scale = i64::from(u);
        let parse = BigDecimal::from_str(s).unwrap();
        let n = parse.with_scale(scale);

        ValueData::Number(NyarNumber::Decimal(n))
    }
}
impl NewNumber<&str> for ValueData {
    fn new_int(s: &str) -> ValueData {
        let n = BigInt::parse_bytes(s.as_bytes(), 10).unwrap();
        ValueData::Number(NyarNumber::Integer(n))
    }

    fn new_dec(s: &str) -> ValueData {
        let n = BigDecimal::from_str(s).unwrap();
        ValueData::Number(NyarNumber::Decimal(n))
    }
}

impl NewNumber<i64> for ValueData {
    fn new_int(i: i64) -> ValueData {
        let n = BigInt::from(i);
        ValueData::Number(NyarNumber::Integer(n))
    }

    fn new_dec(i: i64) -> ValueData {
        let n = BigDecimal::from(i);
        ValueData::Number(NyarNumber::Decimal(n))
    }
}
// endregion
