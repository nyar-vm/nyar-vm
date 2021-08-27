use super::*;
use crate::engine::module::context::decimal_handler::{parse_f32, parse_f64};
use num::{BigInt, BigUint};
use nyar_hir::ast::IntegerLiteral;
use std::{
    collections::BTreeMap,
    fmt::{Debug, Formatter},
};

pub type IntegerHandler = fn(&BigInt) -> Result<Value>;

#[derive(Clone)]
pub struct IntegerHandlerManager {
    handlers: HashMap<String, IntegerHandler>,
}

impl Debug for IntegerHandlerManager {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.handlers.keys()).finish()
    }
}

impl Default for IntegerHandlerManager {
    fn default() -> Self {
        Self { handlers: Default::default() }
    }
}

impl IntegerHandlerManager {
    pub fn register_handler(&mut self, k: &str, v: IntegerHandler) {
        self.handlers.insert(String::from(k), v);
    }
    pub fn parse_integer(&self, literal: &IntegerLiteral) -> Result<Value> {}
}

pub static BUILD_IN_INTEGER_PARSERS: SyncLazy<IntegerHandlerManager> = SyncLazy::new(|| build_integer_parsers());

pub fn build_integer_parsers() -> IntegerHandlerManager {
    let mut handlers = IntegerHandlerManager::default();
    macro_rules! wrap_parser {
        ($h:literal, $t:ty, $v:ident) => {
            handlers.insert($h, |input| Ok(Value::$v(input.parse::<$t>()?)));
        };
    }
    // wrap_parser!("i8", i8, Integer8);
    // wrap_parser!("i16", i16, Integer16);
    // wrap_parser!("i32", i32, Integer32);
    // wrap_parser!("i64", i64, Integer64);
    // wrap_parser!("i128", i128, Integer128);
    // wrap_parser!("isize", isize, IntegerSized);
    //
    // wrap_parser!("u8", u8, UnsignedInteger8);
    // wrap_parser!("u16", u16, UnsignedInteger16);
    // wrap_parser!("u32", u32, UnsignedInteger32);
    // wrap_parser!("u64", u64, UnsignedInteger64);
    // wrap_parser!("u128", u128, UnsignedInteger128);
    // wrap_parser!("usize", usize, UnsignedIntegerSized);

    handlers.register_handler("f32", parse_f32);
    handlers.register_handler("f64", parse_f64);

    handlers.register_handler("int", |input| {
        let i = match BigInt::parse_bytes(input.as_bytes(), 10) {
            Some(s) => s,
            None => {
                return Err(NyarError3::msg("TODO: Int parse error"));
            }
        };
        Ok(Value::Integer(i))
    });
    return handlers;
}
