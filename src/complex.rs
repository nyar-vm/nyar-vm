use num::BigInt;
use super::integer::Integer;
use super::rational::Rational;
use std::ops::Add;
use num::complex as BigComplex;

//type Real = Integer | Rational;
type Base = BigComplex::Complex<BigInt>;


#[derive(Debug, PartialEq, Eq, Clone)]
struct Complex { pub base: Base }

impl Complex {
    fn re(&self) -> Integer {
        Integer::from(&self.base.re)
    }
    fn im(&self) -> Integer {
        Integer::from(&self.base.im)
    }
    fn new(re: &str, im: &str) -> Complex {
        Complex {
            base: Base {
                re: BigInt::parse_bytes(re.as_bytes(), 10).unwrap(),
                im: BigInt::parse_bytes(im.as_bytes(), 10).unwrap(),
            }
        }
    }
}

impl From<&str> for Complex {
    fn from(literal: &str) -> Complex {
        Complex {
            base: Base {
                re: BigInt::from(0),
                im: BigInt::parse_bytes(literal.as_bytes(), 10).unwrap(),
            }
        }
    }
}

impl From<BigInt> for Complex {
    fn from(literal: BigInt) -> Complex {
        Complex {
            base: Base {
                re: literal,
                im: BigInt::from(0),
            }
        }
    }
}

impl From<Integer> for Complex {
    fn from(literal: Integer) -> Complex {
        Complex {
            base: Base {
                re: literal.base,
                im: BigInt::from(0),
            }
        }
    }
}

#[test]
fn test_from() {
    let a = Complex::from("42");
    assert_eq!(a.re(), Integer::from("0"));
    assert_eq!(a.im(), Integer::from("42"));
    let b = Complex::new("42", "0");
    assert_eq!(b.re(), Integer::from("42"));
    assert_eq!(b.im(), Integer::from("0"));
}

//region Middle.Plus
impl Add<Complex> for Complex {
    type Output = Complex;
    fn add(self, other: Complex) -> Self::Output {
        let lhs = self.base;
        let rhs = other.base;
        Complex { base: lhs + rhs }
    }
}

impl Add<Integer> for Complex {
    type Output = Complex;
    fn add(self, other: Integer) -> Self::Output {
        let lhs = self.base;
        let rhs = Complex::from(other.base).base;
        Complex { base: lhs + rhs }
    }
}

#[test]
fn test_add() {
    let a = Complex::from("42");
    let b = Complex::from("8");
    let c = Integer::from("8");
    assert_eq!(a.clone() + b, Complex::from("50"));
    assert_eq!(a.clone() + c, Complex::new("8", "42"));
    let e = Complex::new("42", "8");
    let f = Complex::new("8", "42");
    assert_eq!(e + f, Complex::new("50", "50"));
}
//endregion