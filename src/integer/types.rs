use std::fmt::Debug;
use std::str::FromStr;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct IntegerType<T> {
    pub value: T,
}

pub struct UnsignedType<T> {
    pub value: T,
}

pub type Integer8 = IntegerType<i8>;

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


