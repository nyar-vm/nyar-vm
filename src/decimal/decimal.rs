use crate::traits::ToNative;
use bigdecimal::BigDecimal;
use num::BigInt;
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Decimal {
    pub value: BigDecimal,
}

/*
impl std::ops::Deref for Decimal {
    type Target = BigDecimal;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
*/
// region GC trait
#[cfg(feature = "gc")]
impl gc::Finalize for Decimal {}

// TODO: Trace the bigint inner
#[cfg(feature = "gc")]
unsafe impl gc::Trace for Decimal {
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
impl Decimal {
    pub fn new(decimal: &str, accuracy: &str) -> Decimal {
        let parse = BigDecimal::from_str(decimal).unwrap();
        let acc = i64::from_str(accuracy).unwrap();
        Decimal {
            value: parse.with_scale(acc),
        }
    }
}

// region From trait
impl From<&str> for Decimal {
    fn from(s: &str) -> Self {
        let v = BigDecimal::from_str(s).unwrap();
        Decimal { value: v }
    }
}
impl From<f64> for Decimal {
    fn from(s: f64) -> Self {
        let v = BigDecimal::from(s);
        Decimal { value: v }
    }
}
impl ToNative for Decimal {
    type Output = BigDecimal;
    fn to_native(&self) -> Self::Output {
        self.clone().value
    }
}

// endregion
// region Display trait
impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&self.value).fmt(f)
    }
}

// endregion
// region Operators
macro_rules! wrap_op {
    ($T: ident, $F: ident) => {
        impl $T<Decimal> for Decimal {
            type Output = Decimal;
            fn $F(self, other: Decimal) -> Self::Output {
                let result = self.value.$F(other.value);
                Decimal { value: result }
            }
        }
        impl $T<&Decimal> for Decimal {
            type Output = Decimal;
            fn $F(self, other: &Decimal) -> Self::Output {
                let result = self.value.$F(other.value.clone());
                Decimal { value: result }
            }
        }
        impl $T<Decimal> for &Decimal {
            type Output = Decimal;
            fn $F(self, other: Decimal) -> Self::Output {
                let result = self.value.clone().$F(other.value);
                Decimal { value: result }
            }
        }
        impl $T<&Decimal> for &Decimal {
            type Output = Decimal;
            fn $F(self, other: &Decimal) -> Self::Output {
                let result = self.value.clone().$F(other.value.clone());
                Decimal { value: result }
            }
        }
    };
}

wrap_op!(Add, add);
wrap_op!(Sub, sub);
// endregion
