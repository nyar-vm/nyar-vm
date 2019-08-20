mod shared;

pub use self::shared::Shared;

use crate::NyarClass;
use num::BigInt;
use smartstring::{LazyCompact, SmartString};
use std::{
    collections::LinkedList,
    ops::Deref,
    sync::{Arc, Mutex},
    time::Instant,
};

/// Internal [`NyarValue`] representation.
///
/// Box variants to reduce the size.
#[repr(u8)]
pub enum NyarValue {
    ///
    Void,
    /// The Unit value - [`()`]
    Unit,
    /// A boolean value
    Bool(bool),
    /// An integer value.
    Integer(Shared<BigInt>),
    /// A UTF8 character value
    Character(char),
    /// An [`StringView`] value
    String(Shared<StringView>),
    /// An array value.
    AnyList(Shared<AnyList>),
    /// An blob (byte array).
    AnyDict(Shared<AnyDict>),
    /// A function pointer.
    FunctionPointer,
    /// Any type as a trait object.
    AnyObject,
}

pub struct Float32(f32);

pub struct Float64(f64);

pub struct StringView {
    inner: SmartString<LazyCompact>,
}

pub struct ByteArray {
    inner: Vec<u8>,
}

pub struct AnyList {
    inner: LinkedList<NyarValue>,
}

pub struct AnyDict {
    inner: LinkedList<NyarValue>,
}
