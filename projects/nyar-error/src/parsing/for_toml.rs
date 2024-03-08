use diagnostic::SourceID;
use std::ops::Range;
use toml::de::Error;

use crate::{NyarError, SyntaxError};

impl From<Error> for SyntaxError {
    fn from(value: Error) -> Self {
        match value.span() {
            Some(s) => Self {
                info: value.message().to_string(),
                hint: "".to_string(),
                span: SourceID::default().with_range(Range { start: s.start as u32, end: s.end as u32 }),
            },
            None => Self { info: value.message().to_string(), hint: "".to_string(), span: Default::default() },
        }
    }
}

impl From<Error> for NyarError {
    fn from(value: Error) -> Self {
        SyntaxError::from(value).into()
    }
}
