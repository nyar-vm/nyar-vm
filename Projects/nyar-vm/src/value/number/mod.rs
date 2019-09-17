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
