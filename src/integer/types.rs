use std::fmt::Debug;
use std::str::FromStr;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct IntegerType<T> {
    pub value: T,
}
pub type Integer8 = IntegerType<i8>;
pub type Integer16 = IntegerType<i16>;
pub type Integer32 = IntegerType<i32>;
pub type Integer64 = IntegerType<i64>;
pub type Integer128 = IntegerType<i128>;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct UnsignedType<T> {
    pub value: T,
}
pub type Unsigned8 = UnsignedType<i8>;
pub type Unsigned16 = UnsignedType<i16>;
pub type Unsigned32 = UnsignedType<i32>;
pub type Unsigned64 = UnsignedType<i64>;
pub type Unsigned128 = UnsignedType<i128>;

impl<T> From<&str> for IntegerType<T>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    fn from(i: &str) -> Self {
        let v = T::from_str(i).unwrap();
        IntegerType { value: v }
    }
}

impl<T> From<&str> for UnsignedType<T>
where
    T: FromStr,
    <T as FromStr>::Err: Debug,
{
    fn from(i: &str) -> Self {
        let v = T::from_str(i).unwrap();
        UnsignedType { value: v }
    }
}
