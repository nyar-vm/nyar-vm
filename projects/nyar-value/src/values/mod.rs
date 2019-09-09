use std::{
    collections::{HashMap, LinkedList},
    fmt::{Debug, Display, Formatter},
    ops::{Deref, Not},
    sync::{Arc, Mutex},
    time::Instant,
};

mod byte_array;
mod dict;
use indexmap::IndexMap;
use num::BigInt;
use shredder::{atomic::AtomicGc, Gc, Scan};
use smartstring::{LazyCompact, SmartString};

pub use crate::values::{byte_array::NyarBlob, dict::NyarDict, integer::NyarInteger, listing::NyarList, string::NyarString};
use crate::{NyarCast, NyarClass};

mod integer;
mod listing;
mod string;

/// Internal [`NyarValue`] representation.
///
/// Box variants to reduce the size.
#[repr(u8)]
#[derive(Clone, Scan)]
pub enum NyarValue {
    /// Something wrong happened
    Never,
    /// The unit return
    Unit,
    /// A boolean value
    Bool(bool),
    /// An integer value.
    Unsigned8(u8),
    /// An integer value.
    Unsigned16(u16),
    /// An integer value.
    Unsigned32(u32),
    /// An integer value.
    Unsigned64(u64),
    /// An integer value.
    Unsigned128(u128),
    /// An integer value.
    Integer8(i8),
    /// An integer value.
    Integer16(i16),
    /// An integer value.
    Integer32(i32),
    /// An integer value.
    Integer64(i64),
    /// An integer value.
    Integer128(i128),
    /// An integer value.
    Integer(Gc<NyarInteger>),
    /// An integer value.
    Float32(Float32),
    /// An integer value.
    Float64(Float64),
    /// A UTF8 character value
    Character(char),
    /// An [`StringView`] value
    String(Gc<NyarString>),
    /// An [`StringView`] value
    ByteArray(Gc<NyarBlob>),
    /// An array value.
    AnyList(Gc<NyarList>),
    /// An blob (byte array).
    AnyDict(Gc<NyarDict>),
    /// A function pointer.
    FunctionPointer,
    /// Any type as a trait object.
    AnyObject,
}

#[derive(Copy, Clone, Debug, Scan)]
pub struct Float32(f32);

#[derive(Copy, Clone, Debug, Scan)]
pub struct Float64(f64);

impl Debug for NyarValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NyarValue::Never => f.write_str("!"),
            NyarValue::Unit => f.write_str("()"),
            NyarValue::Bool(v) => Display::fmt(v, f),
            NyarValue::Unsigned8(v) => Display::fmt(v, f),
            NyarValue::Unsigned16(v) => Display::fmt(v, f),
            NyarValue::Unsigned32(v) => Display::fmt(v, f),
            NyarValue::Unsigned64(v) => Display::fmt(v, f),
            NyarValue::Unsigned128(v) => Display::fmt(v, f),
            NyarValue::Integer8(v) => Display::fmt(v, f),
            NyarValue::Integer16(v) => Display::fmt(v, f),
            NyarValue::Integer32(v) => Display::fmt(v, f),
            NyarValue::Integer64(v) => Display::fmt(v, f),
            NyarValue::Integer128(v) => Display::fmt(v, f),
            NyarValue::Integer(v) => Display::fmt(v, f),
            NyarValue::Float32(_) => unimplemented!(),
            NyarValue::Float64(_) => unimplemented!(),
            NyarValue::Character(v) => Display::fmt(v, f),
            NyarValue::String(_) => unimplemented!(),
            NyarValue::ByteArray(_) => unimplemented!(),
            NyarValue::AnyList(_) => unimplemented!(),
            NyarValue::AnyDict(_) => unimplemented!(),
            NyarValue::FunctionPointer => unimplemented!(),
            NyarValue::AnyObject => unimplemented!(),
        }
    }
}

impl NyarCast for NyarValue {
    fn to_value(self) -> NyarValue {
        self
    }

    fn as_i64(&self) -> Option<u16> {
        match self {
            _ => None,
        }
    }
}

impl NyarValue {
    pub fn is_true(&self) -> bool {
        match self {
            NyarValue::Bool(v) => *v,
            _ => false,
        }
    }
    pub fn is_false(&self) -> bool {
        match self {
            NyarValue::Bool(v) => v.not(),
            _ => false,
        }
    }
}
