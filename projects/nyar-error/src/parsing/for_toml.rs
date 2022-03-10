use diagnostic::FileID;
use toml::de::Error;

use crate::{NyarError, SyntaxError};

impl From<Error> for SyntaxError {
    fn from(value: Error) -> Self {
        match value.span() {
            Some(s) => Self { info: value.message().to_string(), hint: "".to_string(), span: FileID::default().with_range(s) },
            None => Self { info: value.message().to_string(), hint: "".to_string(), span: Default::default() },
        }
    }
}

impl From<Error> for NyarError {
    fn from(value: Error) -> Self {
        SyntaxError::from(value).into()
    }
}
