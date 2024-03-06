use crate::{
    encoder::WastEncoder,
    helpers::{EmitConstant, ToWasiType},
    ArrayValue, WasiArrayType, WasiType,
};
use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::Write,
    hash::{Hash, Hasher},
};

mod arithmetic;

pub mod array;
pub mod record;

/// Static values that can be expressed in wasm/wasi
#[derive(Debug, Clone)]
pub enum WasiValue {
    /// The boolean value, `true` or `false`
    Boolean(bool),
    /// The signed 8-bit integer, from `-128` to `127`
    Integer8(i8),
    /// The signed 16-bit integer, from `-32768` to `32767`
    Integer16(i16),
    /// The signed 32-bit integer, from `-2147483648` to `2147483647`
    Integer32(i32),
    /// The signed 64-bit integer, from `-9223372036854775808` to `9223372036854775807`
    Integer64(i64),
    /// The unsigned 8-bit integer, from `0` to `255`
    Unsigned8(u8),
    /// The unsigned 16-bit integer, from `0` to `65535`
    Unsigned16(u16),
    /// The unsigned 32-bit integer, from `0` to `4294967295`
    Unsigned32(u32),
    /// The unsigned 64-bit integer, from `0` to `18446744073709551615`
    Unsigned64(u64),
    /// The 32-bit floating point number
    Float32(f32),
    /// The 64-bit floating point number
    Float64(f64),
    DynamicArray(ArrayValue),
}

impl EmitConstant for WasiValue {
    fn emit_constant<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean(v) => write!(w, "i32.const {}", if *v { 1 } else { 0 })?,
            Self::Integer8(v) => write!(w, "i32.const {}", v)?,
            Self::Integer16(v) => write!(w, "i32.const {}", v)?,
            Self::Integer32(v) => write!(w, "i32.const {}", v)?,
            Self::Integer64(v) => write!(w, "i64.const {}", v)?,
            Self::Unsigned8(v) => write!(w, "i32.const {}", v)?,
            Self::Unsigned16(v) => write!(w, "i32.const {}", v)?,
            Self::Unsigned32(v) => write!(w, "i32.const {}", v)?,
            Self::Unsigned64(v) => write!(w, "i64.const {}", v)?,
            Self::Float32(v) => write!(w, "f32.const {}", v)?,
            Self::Float64(v) => write!(w, "f64.const {}", v)?,
            Self::DynamicArray(v) => v.emit_constant(w)?,
        }
        Ok(())
    }
}
