use super::*;
use bigdecimal::BigDecimal;
use std::{
    fmt::{self, Debug, Formatter},
    mem::transmute,
    num::ParseFloatError,
};

#[derive(Clone)]
pub struct DefaultDecimalHandler {
    handlers: HashMap<String, StringCallback>,
}

impl Debug for DefaultDecimalHandler {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_list().entries(self.handlers.keys()).finish()
    }
}

impl Default for DefaultDecimalHandler {
    fn default() -> Self {
        Self { handlers: Default::default() }
    }
}

impl DefaultDecimalHandler {
    pub fn insert(&mut self, k: &str, v: StringCallback) -> Option<StringCallback> {
        self.handlers.insert(String::from(k), v)
    }
    pub fn parse_decimal(&self, handler: &str, value: &str) -> Result<Value> {
        let parser = match self.handlers.get(handler) {
            Some(s) => s,
            None => {
                return Err(NyarError::msg("TODO: No such dec handler"));
            }
        };
        return parser(value);
    }
}

pub static BUILD_IN_DECIMAL_PARSERS: SyncLazy<DefaultDecimalHandler> = SyncLazy::new(|| build_decimal_parsers());

pub fn build_decimal_parsers() -> DefaultDecimalHandler {
    let mut handlers = DefaultDecimalHandler::default();
    handlers.insert("f32", parse_f32);
    handlers.insert("f64", parse_f64);
    handlers.insert("dec", parse_dec);
    return handlers;
}

pub fn parse_f32(input: &str) -> Result<Value> {
    let v = unsafe { transmute::<f32, [u8; 4]>(input.parse::<f32>()?) };
    Ok(Value::Decimal32(v))
}

pub fn parse_f64(input: &str) -> Result<Value> {
    let v = unsafe { transmute::<f64, [u8; 8]>(input.parse::<f64>()?) };
    Ok(Value::Decimal64(v))
}

pub fn parse_dec(input: &str) -> Result<Value> {
    let i = match BigDecimal::parse_bytes(input.as_bytes(), 10) {
        Some(s) => s,
        None => {
            return Err(NyarError::msg("TODO: Int parse error"));
        }
    };
    Ok(Value::Decimal(box i))
}
