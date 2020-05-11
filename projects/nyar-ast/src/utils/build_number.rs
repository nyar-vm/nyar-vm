use crate::{ast::Number, AST};
use num::{BigInt, BigUint, Num};
use std::fmt::{self, Display};

#[allow(unused_variables)]
impl Number {
    pub fn parse_number(input: AST) -> AST {
        match input.clone() {
            AST::NumberLiteral { handler, data } => {
                let h = handler.as_str();
                match h {
                    "i8" => {
                        let number = data.parse::<i8>().unwrap();
                        return AST::Number(Number::Integer8(number));
                    }
                    "i16" => {
                        let number = data.parse::<i16>().unwrap();
                        return AST::Number(Number::Integer16(number));
                    }
                    "i32" => {
                        let number = data.parse::<i32>().unwrap();
                        return AST::Number(Number::Integer32(number));
                    }
                    "i64" => {
                        let number = data.parse::<i64>().unwrap();
                        return AST::Number(Number::Integer64(number));
                    }
                    "i128" => {
                        let number = data.parse::<i128>().unwrap();
                        return AST::Number(Number::Integer128(number));
                    }
                    "u8" => {
                        let number = data.parse::<u8>().unwrap();
                        return AST::Number(Number::Unsigned8(number));
                    }
                    "u16" => {
                        let number = data.parse::<u16>().unwrap();
                        return AST::Number(Number::Unsigned16(number));
                    }
                    "u32" => {
                        let number = data.parse::<u32>().unwrap();
                        return AST::Number(Number::Unsigned32(number));
                    }
                    "u64" => {
                        let number = data.parse::<u64>().unwrap();
                        return AST::Number(Number::Unsigned64(number));
                    }
                    "u128" => {
                        let number = data.parse::<u128>().unwrap();
                        return AST::Number(Number::Unsigned128(number));
                    }
                    "int" => {
                        let number = BigInt::from_str_radix(&data, 10).unwrap();
                        return AST::Number(Number::Integer(number));
                    }
                    "unt" => {
                        let number = BigUint::from_str_radix(&data, 10).unwrap();
                        return AST::Number(Number::Unsigned(number));
                    }
                    "x" => {
                        let number = BigInt::from_str_radix(&data, 16).unwrap();
                        return AST::Number(Number::Integer(number));
                    }
                    "o" => {
                        let number = BigInt::from_str_radix(&data, 8).unwrap();
                        return AST::Number(Number::Integer(number));
                    }
                    "b" => {
                        let number = BigInt::from_str_radix(&data, 2).unwrap();
                        return AST::Number(Number::Integer(number));
                    }
                    "f32" => {
                        let number = data.parse::<f32>().unwrap();
                        return AST::Number(Number::Decimal32(number));
                    }
                    "f64" => {
                        let number = data.parse::<f64>().unwrap();
                        return AST::Number(Number::Decimal64(number));
                    }
                    _ => (),
                };
            }
            _ => (),
        }
        return input;
    }
}

impl From<AST> for Number {
    fn from(input: AST) -> Self {
        match Number::parse_number(input) {
            AST::Number(i) => i,
            _ => Number::Integer(BigInt::from(0)),
        }
    }
}

impl Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Number::Integer(i) => write!(f, "{}", i),
            Number::Integer8(i) => write!(f, "{}", i),
            Number::Integer16(i) => write!(f, "{}", i),
            Number::Integer32(i) => write!(f, "{}", i),
            Number::Integer64(i) => write!(f, "{}", i),
            Number::Integer128(i) => write!(f, "{}", i),
            Number::Unsigned(i) => write!(f, "{}", i),
            Number::Unsigned8(i) => write!(f, "{}", i),
            Number::Unsigned16(i) => write!(f, "{}", i),
            Number::Unsigned32(i) => write!(f, "{}", i),
            Number::Unsigned64(i) => write!(f, "{}", i),
            Number::Unsigned128(i) => write!(f, "{}", i),
            Number::Decimal32(i) => {
                let mut s = format!("{}", i);
                if !s.ends_with('.') {
                    s.push_str(".0")
                }
                write!(f, "{}", s)
            }
            Number::Decimal64(i) => {
                let mut s = format!("{}", i);
                if !s.ends_with('.') {
                    s.push_str(".0")
                }
                write!(f, "{}", s)
            }
            _ => write!(f, "{:?}", self),
        }
    }
}
