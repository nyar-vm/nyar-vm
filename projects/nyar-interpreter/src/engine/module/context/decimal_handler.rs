use super::*;
use bigdecimal::BigDecimal;

#[derive(Clone, Debug)]
pub struct DefaultDecimalHandler {
    handlers: HashMap<String, StringCallback>,
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
            Some(s) => {s}
            None => {return Err(NyarError::msg("TODO: No such dec handler"))}
        };
        return parser(value)
    }
}

pub static BUILD_DECIMAL_PARSERS: SyncLazy<DefaultDecimalHandler> = SyncLazy::new(|| build_integer_parsers());

pub fn build_integer_parsers() -> DefaultDecimalHandler {
    let mut handlers = DefaultDecimalHandler::default();
    macro_rules! wrap_parser {
        ($h:literal, $t:ty, $v:ident) => {
            handlers.insert($h, |input| Ok(Value::$v(input.parse::<$t>()?)));
        };
    }
    wrap_parser!("f32",i8,Decimal32);
    wrap_parser!("f64",i16,Decimal64);

    handlers.insert("dec", |input| {
        let i = match BigDecimal::parse_bytes(input.as_bytes(), 10) {
            Some(s) => s,
            None => {
                return Err(NyarError::msg("TODO: Int parse error"));
            }
        };
        Ok(Value::Decimal(box i))
    });

    return handlers
}