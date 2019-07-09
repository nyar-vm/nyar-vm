use num::BigInt;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Integer {
    value: BigInt,
}

impl std::ops::Deref for Integer {
    type Target = BigInt;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// region From trait
impl From<&str> for Integer {
    fn from(literal: &str) -> Self {
        let v = BigInt::parse_bytes(literal.as_bytes(), 10).unwrap();
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
    //! from byte form
    fn from_base(literal: &str, base: u32) -> Integer {
        let v = BigInt::parse_bytes(literal.as_bytes(), base).unwrap();
        Integer { value: v }
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

#[cfg(test)]
mod tests {
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
