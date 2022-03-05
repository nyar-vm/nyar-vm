use toml::de::Error;

use crate::{FileSpan, NyarError, SyntaxError};

impl From<Error> for SyntaxError {
    fn from(value: Error) -> Self {
        match value.span() {
            Some(s) => Self { info: value.message().to_string(), span: FileSpan { file: 0, head: s.start, tail: s.end } },
            None => Self { info: value.message().to_string(), span: Default::default() },
        }
    }
}

impl From<Error> for NyarError {
    fn from(value: Error) -> Self {
        SyntaxError::from(value).into()
    }
}
