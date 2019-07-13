use crate::Integer;
use num::rational::BigRational;
use num::BigInt;

use std::ops::Add;

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Rational {
    pub value: BigRational,
}

impl std::ops::Deref for Rational {
    type Target = BigRational;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl Rational {
    pub fn numerator(&self) -> Integer {
        <Integer as std::convert::From<num::BigInt>>::from(self.value.numer().clone())
    }
    pub fn denominator(&self) -> Integer {
        <Integer as std::convert::From<num::BigInt>>::from(self.value.denom().clone())
    }
    pub fn new(numerator: &str, denominator: &str) -> Rational {
        Rational {
            value: BigRational::new(
                BigInt::parse_bytes(numerator.as_bytes(), 10).unwrap(),
                BigInt::parse_bytes(denominator.as_bytes(), 10).unwrap(),
            ),
        }
    }
    pub fn from_integer(numerator: BigInt, denominator: BigInt) -> Rational {
        Rational {
            value: BigRational::new(numerator, denominator),
        }
    }
}

impl From<&str> for Rational {
    fn from(s: &str) -> Self {
        let i = BigInt::parse_bytes(s.as_bytes(), 10).unwrap();
        Rational::from_integer(i, BigInt::from(1))
    }
}

impl From<i32> for Rational {
    fn from(i: i32) -> Self {
        Rational::from_integer(BigInt::from(i), BigInt::from(1))
    }
}

impl From<BigInt> for Rational {
    fn from(i: BigInt) -> Self {
        Rational::from_integer(i, BigInt::from(1))
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

#[test]
fn test_from() {
    let a = Rational::new("4", "2");
    assert_eq!(a.numerator(), Integer::from(2));
    assert_eq!(a.denominator(), Integer::from(1));
    let b = Rational::from(2);
    assert_eq!(b.numerator(), Integer::from(2));
    assert_eq!(b.denominator(), Integer::from(1));
}

#[test]
fn test_add() {
    let a = Rational::new("8", "6");
    let b = Rational::new("4", "2");
    let c = Integer::from("2");
    assert_eq!(a.clone() + b, Rational::new("10", "3"));
    assert_eq!(a.clone() + c, Rational::new("10", "3"));
}
