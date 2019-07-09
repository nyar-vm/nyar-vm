use crate::real::{Real, Integer};


use num::BigInt;
use num::rational::BigRational;
use std::ops::Add;

impl Add<Integer> for Integer {
    type Output = Integer;
    fn add(self, other: Integer) -> Self::Output {
        let lhs = self.value;
        let rhs = other.value;
        let result = lhs + rhs;
        Real { value: result }
    }
}


// region From trait
impl From<&str> for Integer {
    fn from(literal: &str) -> Self {
        let v = BigInt::parse_bytes(literal.as_bytes(), 10).unwrap();
        Real { value: v }
    }
}

impl From<i32> for Integer {
    fn from(i: i32) -> Self {
        Real { value: BigInt::from(i) }
    }
}

impl From<BigInt> for Integer {
    fn from(i: BigInt) -> Self {
        Real { value: i }
    }
}

impl Integer {
    //! from byte form
    fn from_base(literal: &str, base: u32) -> Integer {
        let v = BigInt::parse_bytes(literal.as_bytes(), base).unwrap();
        Real { value: v }
    }
    fn from_x(literal: &str) -> Integer {
        Integer::from_base(literal, 16u32)
    }
    fn from_o(literal: &str) -> Integer {
        Integer::from_base(literal, 8u32)
    }
    fn from_b(literal: &str) -> Integer {
        Integer::from_base(literal, 2u32)
    }
}
// endregion

#[test]
fn run() {
    let a: Integer = Integer::from_x("20045600af5604564560000");
    let b: Integer = Integer::from(2000000000);
    println!("{}", a.value);
    panic!()
}
