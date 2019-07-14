use crate::Rational;
use num::BigInt;
use std::fmt;
use std::ops::{Add, Div, Mul, Rem, Sub};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Integer {
    pub value: BigInt,
}

// region
impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        BigInt::fmt(&self.value, f)
    }
}

/*
impl std::ops::Deref for Integer {
    type Target = BigInt;
    fn deref(&self) -> &Self::Target { &self.value }
}
*/
// endregion
// region GC trait
#[cfg(feature = "gc")]
impl gc::Finalize for Integer {}

#[cfg(feature = "gc")]
unsafe impl gc::Trace for Integer {
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

// endregion
// region From trait
impl From<&str> for Integer {
    fn from(s: &str) -> Self {
        let v = BigInt::parse_bytes(s.as_bytes(), 10).unwrap();
        Integer { value: v }
    }
}

impl From<i32> for Integer {
    fn from(i: i32) -> Self {
        Integer {
            value: BigInt::from(i),
        }
    }
}

impl From<BigInt> for Integer {
    fn from(i: BigInt) -> Self {
        Integer { value: i }
    }
}

impl Integer {
    pub fn new(integer: &str) -> Integer {
        Integer::from(integer)
    }
    pub fn from_base(literal: &str, base: u32) -> Integer {
        let v = BigInt::parse_bytes(literal.as_bytes(), base).unwrap();
        Integer { value: v }
    }
    pub fn from_x(s: &str) -> Integer {
        Integer::from_base(s, 16u32)
    }
    pub fn from_o(s: &str) -> Integer {
        Integer::from_base(s, 8u32)
    }
    pub fn from_b(s: &str) -> Integer {
        Integer::from_base(s, 2u32)
    }
}

// endregion
// region Operators
impl PartialEq<i32> for Integer {
    fn eq(&self, other: &i32) -> bool {
        self.value == BigInt::from(*other)
    }
    fn ne(&self, other: &i32) -> bool {
        !self.eq(other)
    }
}

impl Add<Integer> for Integer {
    type Output = Integer;
    fn add(self, other: Integer) -> Integer {
        let result = self.value + other.value;
        Integer::from(result)
    }
}

impl Sub<Integer> for Integer {
    type Output = Integer;
    fn sub(self, other: Integer) -> Integer {
        let result = self.value - other.value;
        Integer::from(result)
    }
}

impl Mul<Integer> for Integer {
    type Output = Integer;
    fn mul(self, other: Integer) -> Integer {
        let result = self.value * other.value;
        Integer::from(result)
    }
}

impl Div<Integer> for Integer {
    type Output = Rational;
    fn div(self, other: Integer) -> Rational {
        Rational::from_pair(self.value, other.value)
    }
}

impl Rem<Integer> for Integer {
    type Output = Integer;
    fn rem(self, other: Integer) -> Integer {
        let result = self.value % other.value;
        Integer::from(result)
    }
}

// endregion

#[test]
fn no_syntax_error() {}