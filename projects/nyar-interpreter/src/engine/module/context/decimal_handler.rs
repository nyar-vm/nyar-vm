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

pub static BUILD_IN_DECIMAL_PARSERS: SyncLazy<DefaultDecimalHandler> = SyncLazy::new(|| unsafe { build_decimal_parsers() });

pub unsafe fn build_decimal_parsers() -> DefaultDecimalHandler {
    let mut handlers = DefaultDecimalHandler::default();

    handlers.insert("f32", |input| {
        let v = input.parse::<f32>()?;
        Ok(Value::Decimal32(transmute::<f32, [u8; 4]>(v)))
    });

    handlers.insert("f64", |input| {
        let v = input.parse::<f64>()?;
        Ok(Value::Decimal64(transmute::<f64, [u8; 8]>(v)))
    });

    handlers.insert("dec", |input| {
        let i = match BigDecimal::parse_bytes(input.as_bytes(), 10) {
            Some(s) => s,
            None => {
                return Err(NyarError::msg("TODO: Int parse error"));
            }
        };
        Ok(Value::Decimal(box i))
    });

    return handlers;
}
