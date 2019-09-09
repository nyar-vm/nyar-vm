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
