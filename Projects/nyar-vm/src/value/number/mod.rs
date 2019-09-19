use bigdecimal::BigDecimal;
use gc;
use num::BigInt;
use std::fmt::{self, Display};

#[derive(Debug, Clone)]
pub enum Number {
    Integer(BigInt),
    Integer8(i8),
    Integer16(i16),
    Integer32(i32),
    Integer64(i64),
    Integer128(i128),
    //Unsigned(BigUint),
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
    Unsigned128(u128),
    Decimal(BigDecimal),
    Float32(f32),
    Float64(f64),
}

impl gc::Finalize for Number {}
unsafe impl gc::Trace for Number {
    #[inline]
    unsafe fn trace(&self) {}
    #[inline]
    unsafe fn root(&self) {}
    #[inline]
    unsafe fn unroot(&self) {}
    #[inline]
    fn finalize_glue(&self) {
        gc::Finalize::finalize(self)
    }
}

impl Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Number::Integer(ref v) => write!(f, "{}", v),
            Number::Integer8(ref v) => write!(f, "{}i8", v),
            Number::Integer16(ref v) => write!(f, "{}i16", v),
            Number::Integer32(ref v) => write!(f, "{}i32", v),
            Number::Integer64(ref v) => write!(f, "{}i64", v),
            Number::Integer128(ref v) => write!(f, "{}i128", v),
            Number::Unsigned8(ref v) => write!(f, "{}u8", v),
            Number::Unsigned16(ref v) => write!(f, "{}u16", v),
            Number::Unsigned32(ref v) => write!(f, "{}u32", v),
            Number::Unsigned64(ref v) => write!(f, "{}u64", v),
            Number::Unsigned128(ref v) => write!(f, "{}u128", v),
            Number::Decimal(ref v) => write!(f, "{}", v),
            Number::Float32(ref v) => write!(f, "{}f32", v),
            Number::Float64(ref v) => write!(f, "{}f64", v),
        }
    }
}


macro_rules! wrap_type {
    ($T:ty, $F:ident) => {
        impl From<$T> for Number {
            fn from(i: $T) -> Number {
                Number::$F(i)
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
