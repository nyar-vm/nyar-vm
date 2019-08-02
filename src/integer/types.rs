use super::integer::Integer;
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

impl<T> PartialEq<T> for NativeType<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &T) -> bool {
        self.value == *other
    }
}

impl<T> Add<NativeType<T>> for NativeType<T>
where
    T: Add<Output = T> + Copy + Clone,
{
    type Output = NativeType<T>;
    fn add(self, other: NativeType<T>) -> Self::Output {
        let result = self.value + other.value;
        NativeType { value: result }
    }
}

impl<T> Add<T> for NativeType<T>
where
    T: Add<Output = T> + Copy + Clone,
{
    type Output = NativeType<T>;
    fn add(self, other: T) -> Self::Output {
        let result = self.value + other;
        NativeType { value: result }
    }
}

impl<T> Sub<NativeType<T>> for NativeType<T>
where
    T: Sub<Output = T> + Copy + Clone,
{
    type Output = NativeType<T>;
    fn sub(self, other: NativeType<T>) -> Self::Output {
        let result = self.value - other.value;
        NativeType { value: result }
    }
}

impl<T> Sub<T> for NativeType<T>
where
    T: Sub<Output = T> + Copy + Clone,
{
    type Output = NativeType<T>;
    fn sub(self, other: T) -> Self::Output {
        let result = self.value - other;
        NativeType { value: result }
    }
}
