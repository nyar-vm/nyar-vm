mod integer;
mod shared;
mod string;

pub use self::shared::Shared;

use crate::{values::integer::NyarInteger, NyarClass};
use gc_derive::{Finalize, Trace};
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
    Integer(Shared<NyarInteger>),
    /// A UTF8 character value
    Character(char),
    /// An [`StringView`] value
    String(Shared<String>),
    /// An array value.
    AnyList(Shared<AnyList>),
    /// An blob (byte array).
    AnyDict(Shared<AnyDict>),
    /// A function pointer.
    FunctionPointer,
    /// Any type as a trait object.
    AnyObject,
}

#[derive(Debug, Trace, Finalize)]
pub struct Float32(f32);

#[derive(Trace, Debug)]
pub struct Float64(f64);

#[derive(Trace, Debug)]
pub struct ByteArray {
    inner: Vec<u8>,
}

#[derive(Trace, Debug)]
pub struct AnyList {
    inner: LinkedList<NyarValue>,
}

#[derive(Trace, Debug)]
pub struct AnyDict {
    inner: LinkedList<NyarValue>,
}
