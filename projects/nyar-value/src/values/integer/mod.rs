use std::{
    fmt::{Display, Formatter},
    rc::Rc,
    sync::Arc,
};

use num::{BigInt, Integer, One, Zero};
use shredder::{
    marker::{GcDrop, GcSafe},
    Scan, Scanner,
};

use nyar_macro::sync_value_type;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NyarInteger {
    inner: BigInt,
}

sync_value_type!(NyarInteger);

impl NyarInteger {
    /// Create a new [`NyarInteger`].
    #[inline]
    pub fn new<T: Into<BigInt>>(value: T) -> Self {
        Self { inner: value.into() }
    }

    /// Create a [`NyarInteger`] with value `0`.
    #[inline]
    pub fn zero() -> Self {
        Self { inner: BigInt::zero() }
    }

    /// Check if value is zero
    #[inline]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Create a [`NyarInteger`] with value `1`.
    #[inline]
    pub fn one() -> Self {
        Self { inner: BigInt::one() }
    }

    /// Check if is one.
    #[inline]
    pub fn is_one(&self) -> bool {
        self.inner.is_one()
    }

    /// Convert bigint to string with radix.
    #[inline]
    pub fn to_string_radix(&self, radix: u32) -> String {
        self.inner.to_str_radix(radix)
    }

    /// Converts a string to a `BigInt` with the specified radix.
    #[inline]
    pub fn from_string_radix(input: &str, radix: u32) -> Option<Self> {
        Some(Self { inner: BigInt::parse_bytes(input.as_bytes(), radix)? })
    }

    #[inline]
    pub fn mod_floor(x: &Self, y: &Self) -> Self {
        Self::new(x.inner.mod_floor(&y.inner))
    }

    #[inline]
    pub(crate) fn as_inner(&self) -> &BigInt {
        &self.inner
    }
}

impl Display for NyarInteger {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}
