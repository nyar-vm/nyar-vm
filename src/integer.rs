use num::BigInt;
use std::ops::Add;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Integer {
    pub value: BigInt,
}


impl std::ops::Deref for Integer {
    type Target = BigInt;
    fn deref(&self) -> &Self::Target { &self.value }
}
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
        Finalize::finalize(self)
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
    fn new(integer: &str) -> Integer {
        Integer::from(integer)
    }
    fn from_base(literal: &str, base: u32) -> Integer {
        let v = BigInt::parse_bytes(literal.as_bytes(), base).unwrap();
        Integer { value: v }
    }
    fn from_x(s: &str) -> Integer {
        Integer::from_base(s, 16u32)
    }
    fn from_o(s: &str) -> Integer {
        Integer::from_base(s, 8u32)
    }
    fn from_b(s: &str) -> Integer {
        Integer::from_base(s, 2u32)
    }
}

// endregion
// region Operators
impl Add<Integer> for Integer {
    type Output = Integer;
    fn add(self, other: Integer) -> Integer {
        let lhs = self.value;
        let rhs = other.value;
        let result = lhs + rhs;
        Integer { value: result }
    }
}
// endregion


#[cfg(test)]
mod testing_covert {
    use super::*;

    #[test]
    fn from_hex() {
        let lhs = Integer::from_x("FF");
        let rhs = Integer::from(255);
        assert_eq!(lhs, rhs)
    }

    #[test]
    fn from_oct() {
        let lhs = Integer::from_o("77");
        let rhs = Integer::from(63);
        assert_eq!(lhs, rhs)
    }

    #[test]
    fn from_bin() {
        let lhs = Integer::from_b("11");
        let rhs = Integer::from(3);
        assert_eq!(lhs, rhs)
    }

    #[test]
    fn from_str() {
        let lhs = Integer::from("99");
        let rhs = Integer::from(99);
        assert_eq!(lhs, rhs)
    }
}

#[cfg(test)]
mod testing_operator {
    use super::*;

    #[test]
    fn add() {
        let lhs = Integer::from(2);
        let rhs = Integer::from(3);
        assert_eq!(lhs + rhs, Integer::from(5))
    }
}
