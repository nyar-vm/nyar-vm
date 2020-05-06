use crate::{ast::Number, AST};
use num::{BigInt, BigUint, Num};

#[allow(unused_variables)]
impl Number {
    pub fn parse_number(input: AST) -> AST {
        match input.clone() {
            AST::NumberLiteral { handler, data } => {
                let h = handler.as_str();
                match h {
                    "i8" => {
                        let number = data.parse::<i8>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "i16" => {
                        let number = data.parse::<i16>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "i32" => {
                        let number = data.parse::<i32>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "i64" => {
                        let number = data.parse::<i64>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "i128" => {
                        let number = data.parse::<i128>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "u8" => {
                        let number = data.parse::<u8>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "u16" => {
                        let number = data.parse::<u16>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "u32" => {
                        let number = data.parse::<u32>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "u64" => {
                        let number = data.parse::<u64>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "u128" => {
                        let number = data.parse::<u128>().unwrap();
                        return AST::Number(Number(number));
                    }
                    "int" => {
                        let number = BigInt::from_str_radix(&data, 10).unwrap();
                        return AST::Number(Number(number));
                    }
                    "unt" => {
                        let number = BigUint::from_str_radix(&data, 10).unwrap();
                        return AST::Number(Number(number));
                    }
                    "x" => {
                        let number = BigInt::from_str_radix(&data, 16).unwrap();
                        return AST::Number(Number(number));
                    }
                    "o" => {
                        let number = BigInt::from_str_radix(&data, 8).unwrap();
                        return AST::Number(Number(number));
                    }
                    "b" => {
                        let number = BigInt::from_str_radix(&data, 2).unwrap();
                        return AST::Number(Number(number));
                    }
                    _ => (),
                };
            }
            _ => (),
        }
        return input;
    }
}
