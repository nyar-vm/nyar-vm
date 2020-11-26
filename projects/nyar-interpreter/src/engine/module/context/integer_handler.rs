use super::*;
use num::{BigInt, BigUint};
use std::fmt::{Debug, Formatter};

#[derive(Clone)]
pub struct DefaultIntegerHandler {
    handlers: HashMap<String, StringCallback>,
}

impl Debug for DefaultIntegerHandler {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.handlers.keys()).finish()
    }
}

impl Default for DefaultIntegerHandler {
    fn default() -> Self {
        Self { handlers: Default::default() }
    }
}

impl DefaultIntegerHandler {
    pub fn insert(&mut self, k: &str, v: StringCallback) -> Option<StringCallback> {
        self.handlers.insert(String::from(k), v)
    }
    pub fn parse_integer(&self, handler: &str, value: &str) -> Result<Value> {
        let parser = match self.handlers.get(handler) {
            Some(s) => s,
            None => return Err(NyarError::int_handler_not_found(handler, None)),
        };
        return parser(value);
    }
}

pub static BUILD_IN_INTEGER_PARSERS: SyncLazy<DefaultIntegerHandler> = SyncLazy::new(|| build_integer_parsers());

pub fn build_integer_parsers() -> DefaultIntegerHandler {
    let mut handlers = DefaultIntegerHandler::default();
    macro_rules! wrap_parser {
        ($h:literal, $t:ty, $v:ident) => {
            handlers.insert($h, |input| Ok(Value::$v(input.parse::<$t>()?)));
        };
    }
    wrap_parser!("i8", i8, Integer8);
    wrap_parser!("i16", i16, Integer16);
    wrap_parser!("i32", i32, Integer32);
    wrap_parser!("i64", i64, Integer64);
    wrap_parser!("i128", i128, Integer128);
    wrap_parser!("isize", isize, IntegerSized);

    wrap_parser!("u8", u8, UnsignedInteger8);
    wrap_parser!("u16", u16, UnsignedInteger16);
    wrap_parser!("u32", u32, UnsignedInteger32);
    wrap_parser!("u64", u64, UnsignedInteger64);
    wrap_parser!("u128", u128, UnsignedInteger128);
    wrap_parser!("usize", usize, UnsignedIntegerSized);

    handlers.insert("f32", parse_f32);
    handlers.insert("f64", parse_f64);

    handlers.insert("int", |input| {
        let i = match BigInt::parse_bytes(input.as_bytes(), 10) {
            Some(s) => s,
            None => {
                return Err(NyarError::msg("TODO: Int parse error"));
            }
        };
        Ok(Value::Integer(box i))
    });

    handlers.insert("uint", |input| {
        let i = match BigUint::parse_bytes(input.as_bytes(), 10) {
            Some(s) => s,
            None => {
                return Err(NyarError::msg("TODO: Dec parse error"));
            }
        };
        Ok(Value::UnsignedInteger(i))
    });
    return handlers;
}
