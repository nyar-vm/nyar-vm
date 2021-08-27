use super::*;
use bigdecimal::BigDecimal;
use std::{
    fmt::{self, Debug, Formatter},
    mem::transmute,
};

#[derive(Clone)]
pub struct DefaultDecimalHandler {
    handlers: HashMap<String, StringCallback>,
}

impl Debug for DefaultDecimalHandler {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
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
    pub fn parse_decimal(&self, handler: &str, value: &str) -> NyarResult<Value> {
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

pub fn parse_f32(input: &str) -> NyarResult<Value> {
    Err(NyarError::msg("unimplemented"))
    // Ok(Value::Decimal32(FloatWrapper::new(input.parse::<f32>()?)))
}

pub fn parse_f64(input: &str) -> NyarResult<Value> {
    Err(NyarError::msg("unimplemented"))
    // Ok(Value::Decimal64(FloatWrapper::new(input.parse::<f64>()?)))
}

pub fn parse_dec(input: &str) -> NyarResult<Value> {
    match BigDecimal::parse_bytes(input.as_bytes(), 10) {
        Some(s) => Ok(Value::Decimal(s)),
        None => Err(NyarError::syntax_error("Can not parse `{}` as decimal")),
    }
}
