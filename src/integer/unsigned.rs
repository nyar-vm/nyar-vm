use num::BigUInt;
use num::Integer;
use std::fmt;
use std::ops::{Add, Div, Mul, Rem, Sub};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Unsigned {
    pub value: BigUInt,
}