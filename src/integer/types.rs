use super::integer::Integer;
use num::BigInt;
use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Rem, Sub};
use std::str::FromStr;

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct NativeType<T> {
    pub value: T,
}

pub type Integer8 = NativeType<i8>;
pub type Integer16 = NativeType<i16>;
pub type Integer32 = NativeType<i32>;
pub type Integer64 = NativeType<i64>;
pub type Integer128 = NativeType<i128>;

pub type Unsigned8 = NativeType<i8>;
pub type Unsigned16 = NativeType<i16>;
pub type Unsigned32 = NativeType<i32>;
pub type Unsigned64 = NativeType<i64>;
pub type Unsigned128 = NativeType<i128>;

// region From
impl<T> From<&str> for NativeType<T>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    fn from(i: &str) -> Self {
        let v = T::from_str(i).unwrap();
        NativeType { value: v }
    }
}

impl<T> From<NativeType<T>> for Integer
where
    num::BigInt: std::convert::From<T>,
{
    fn from(i: NativeType<T>) -> Self {
        let v = num::BigInt::from(i.value);
        Integer { value: v }
    }
}

/* IDEA lint broken
macro_rules! warp_native {
    ($T:ty) => {
        impl From<$T> for NativeType<$T> {
            fn from(i: $T) -> Self {
                NativeType { value: i }
            }
        }
    };
    ($($x:ty),*) => {
        $(warp_native!($x);)*
    };
}

warp_native! {i8,i16,i32,i64,i128}
warp_native! {u8,u16,u32,u64,u128}
*/
// endregion
// region OPs
impl<T> PartialEq<T> for NativeType<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &T) -> bool {
        self.value == *other
    }
}

macro_rules! warp_op {
    ($T: ident, $F: ident) => {
        impl<T> $T<NativeType<T>> for NativeType<T>
        where
            T: $T<Output = T> + Copy + Clone,
        {
            type Output = NativeType<T>;
            fn $F(self, other: NativeType<T>) -> Self::Output {
                let result = self.value.$F(other.value);
                NativeType { value: result }
            }
        }
        impl<T> $T<T> for NativeType<T>
        where
            T: $T<Output = T> + Copy + Clone,
        {
            type Output = NativeType<T>;
            fn $F(self, other: T) -> Self::Output {
                let result = self.value.$F(other);
                NativeType { value: result }
            }
        }
    };
}

warp_op!(Add, add);
warp_op!(Sub, sub);
warp_op!(Mul, mul);
warp_op!(Div, div);
// endregion
