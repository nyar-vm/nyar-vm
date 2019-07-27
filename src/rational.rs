use crate::Integer;
use num::rational::BigRational;
use num::BigInt;

use std::fmt;
use std::ops::{Add, Sub};

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Rational {
    pub value: BigRational,
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        BigRational::fmt(&self.value, f)
    }
}

// region GC trait
#[cfg(feature = "gc")]
impl gc::Finalize for Rational {}

// TODO: trace two bigint, rather than create a new one
#[cfg(feature = "gc")]
unsafe impl gc::Trace for Rational {
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
impl Rational {
    pub fn numerator(&self) -> Integer {
        let n = self.value.numer().clone();
        Integer { value: n }
    }
    pub fn denominator(&self) -> Integer {
        let n = self.value.denom().clone();
        Integer { value: n }
    }
    pub fn new(numerator: &str, denominator: &str) -> Rational {
        let n = BigInt::parse_bytes(numerator.as_bytes(), 10).unwrap();
        let d = BigInt::parse_bytes(denominator.as_bytes(), 10).unwrap();
        Rational {
            value: BigRational::new(n, d),
        }
    }
    pub fn from_pair(numerator: BigInt, denominator: BigInt) -> Rational {
        Rational {
            value: BigRational::new(numerator, denominator),
        }
    }
    pub fn from_float(f: f64) -> Rational {
        let r = BigRational::from_float(f).unwrap();
        Rational { value: r }
    }
}

impl From<&str> for Rational {
    fn from(s: &str) -> Self {
        let i = BigInt::parse_bytes(s.as_bytes(), 10).unwrap();
        Rational::from_pair(i, BigInt::from(1))
    }
}

impl From<i64> for Rational {
    fn from(i: i64) -> Self {
        Rational::from_pair(BigInt::from(i), BigInt::from(1))
    }
}

impl From<BigInt> for Rational {
    fn from(i: BigInt) -> Self {
        Rational::from_pair(i, BigInt::from(1))
    }
}

impl From<Integer> for Rational {
    fn from(i: Integer) -> Self {
        Rational::from_pair(i.value, BigInt::from(1))
    }
}

impl Add<Rational> for Rational {
    type Output = Rational;
    fn add(self, other: Rational) -> Self::Output {
        let lhs = self.value;
        let rhs = other.value;
        Rational { value: lhs + rhs }
    }
}

impl Add<Integer> for Rational {
    type Output = Rational;
    fn add(self, other: Integer) -> Self::Output {
        let lhs = self.value;
        let rhs = Rational::from(other.value).value;
        Rational { value: lhs + rhs }
    }
}

impl Sub<Rational> for Rational {
    type Output = Rational;
    fn sub(self, other: Rational) -> Self::Output {
        let lhs = self.value;
        let rhs = other.value;
        Rational { value: lhs - rhs }
    }
}

impl Sub<Integer> for Rational {
    type Output = Rational;
    fn sub(self, other: Integer) -> Self::Output {
        let lhs = self.value;
        let rhs = Rational::from(other.value).value;
        Rational { value: lhs - rhs }
    }
}
