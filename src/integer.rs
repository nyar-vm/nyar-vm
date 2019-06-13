use num::BigInt;
use std::ops::{Add, Sub, Mul, Div};
use super::rational::{Rational};

type Base = num::BigInt;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Integer { pub base: Base }

//region From Literal
impl From<&str> for Integer {
    fn from(literal: &str) -> Self {
        Integer {
            base: BigInt::parse_bytes(literal.as_bytes(), 10).unwrap(),
        }
    }
}

impl From<Base> for Integer {
    fn from(inner: Base) -> Self { Integer { base: inner } }
}

impl From<&Base> for Integer {
    fn from(inner: &Base) -> Self { Integer { base: inner.clone() } }
}

impl From<Integer> for String {
    //! as string
    fn from(i: Integer) -> Self { i.base.to_str_radix(10) }
}


impl Integer {
    //! from byte form
    fn from_x(literal: &str) -> Integer {
        Integer {
            base: BigInt::parse_bytes(literal.as_bytes(), 16).unwrap(),
        }
    }
    fn from_o(literal: &str) -> Integer {
        Integer {
            base: BigInt::parse_bytes(literal.as_bytes(), 8).unwrap(),
        }
    }
    fn from_b(literal: &str) -> Integer {
        Integer {
            base: BigInt::parse_bytes(literal.as_bytes(), 2).unwrap(),
        }
    }
}

#[test]
fn test_from() {
    assert_eq!(BigInt::from(0), Integer::from("0").base);
    assert_eq!(Integer::from_x("10"), Integer::from("16"));
    assert_eq!(Integer::from_o("10"), Integer::from("8"));
    assert_eq!(Integer::from_b("10"), Integer::from("2"));
}
//endregion

//region Middle.Plus
impl Add<Integer> for Integer {
    type Output = Integer;
    fn add(self, other: Integer) -> Self::Output {
        let lhs = self.base;
        let rhs = other.base;
        Integer { base: lhs + rhs }
    }
}

#[test]
fn test_add() {
    let a = Integer::from("42");
    let b = Integer::from("8");
    assert_eq!(a + b, Integer::from("50"));
}

//endregion
//region Middle.Minus
impl Sub<Integer> for Integer {
    type Output = Integer;
    fn sub(self, other: Integer) -> Self::Output {
        let lhs = self.base;
        let rhs = other.base;
        Integer { base: lhs - rhs }
    }
}

#[test]
fn test_sub() {
    let a = Integer::from("42");
    let b = Integer::from("8");
    assert_eq!(a.clone() - b.clone(), Integer::from("+34"));
    assert_eq!(b.clone() - a.clone(), Integer::from("-34"));
}

//endregion
//region Middle.Multiply
impl Mul<Integer> for Integer {
    type Output = Integer;
    #[inline]
    fn mul(self, other: Integer) -> Self::Output {
        let lhs = self.base;
        let rhs = other.base;
        Integer { base: lhs * rhs }
    }
}

#[test]
fn test_mul() {
    let a = Integer::from("42");
    let b = Integer::from("+8");
    let c = Integer::from("-8");
    assert_eq!(a.clone() * b, Integer::from("+336"));
    assert_eq!(a.clone() * c, Integer::from("-336"));
}

//endregion
//region Middle.Div
impl Div<Integer> for Integer {
    type Output = Rational;
    #[inline]
    fn div(self, other: Integer) -> Self::Output {
        let lhs = self.base;
        let rhs = other.base;
        Rational {
            base: num::rational::Ratio::new(lhs, rhs)
        }
    }
}

#[test]
fn test_div() {
    let a = Integer::from("8");
    let b = Integer::from("6");
    let c = a / b;
    assert_eq!(c.up(), Integer::from("4"));
    assert_eq!(c.down(), Integer::from("3"));
}
//endregion


pub static ZERO: Integer = Integer::from("0");
pub static ONE: Integer = Integer::from("1");