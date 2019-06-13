use num::rational::Ratio as BigRational;
use num::BigInt;
use super::integer::Integer;
use std::ops::Add;

type Base = BigRational<BigInt>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Rational { pub base: Base }

impl Rational {
    pub fn up(&self) -> Integer {
        Integer::from(self.base.numer())
    }
    pub fn down(&self) -> Integer {
        Integer::from(self.base.denom())
    }
    pub fn new(numerator: &str, denominator: &str) -> Rational {
        Rational {
            base: Base::new(
                BigInt::parse_bytes(numerator.as_bytes(), 10).unwrap(),
                BigInt::parse_bytes(denominator.as_bytes(), 10).unwrap(),
            )
        }
    }
    pub fn from_i(numerator: BigInt, denominator: BigInt) -> Rational {
        Rational {
            base: Base::new(numerator, denominator)
        }
    }
}

/*
impl From<&str> for Rational {
    fn from(s: &str) -> Self {
        let i = BigInt::parse_bytes(s.as_bytes(), 10).unwrap();
        Rational { base: Base::from_integer(i) }
    }
}
*/

impl From<BigInt> for Rational {
    fn from(integer: BigInt) -> Self {
        Rational { base: Base::from_integer(integer) }
    }
}

#[test]
fn test_from() {
    let a = Rational::new("4", "2");
    assert_eq!(a.up(), Integer::from("2"));
    assert_eq!(a.down(), Integer::from("1"));
    let b = Rational::from(BigInt::from(2));
    assert_eq!(b.up(), Integer::from("2"));
    assert_eq!(b.down(), Integer::from("1"));
}

impl Add<Rational> for Rational {
    type Output = Rational;
    fn add(self, other: Rational) -> Self::Output {
        let lhs = self.base;
        let rhs = other.base;
        Rational { base: lhs + rhs }
    }
}

impl Add<Integer> for Rational {
    type Output = Rational;
    fn add(self, other: Integer) -> Self::Output {
        let lhs = self.base;
        let rhs = Rational::from(other.base).base;
        Rational { base: lhs + rhs }
    }
}

#[test]
fn test_add() {
    let a = Rational::new("8", "6");
    let b = Rational::new("4", "2");
    let c = Integer::from("2");
    assert_eq!(a.clone() + b, Rational::new("10", "3"));
    assert_eq!(a.clone() + c, Rational::new("10", "3"));
}

