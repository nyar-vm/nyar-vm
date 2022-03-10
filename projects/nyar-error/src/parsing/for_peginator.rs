use peginator::ParseError;

use super::*;

impl From<ParseError> for SyntaxError {
    fn from(e: ParseError) -> Self {
        SyntaxError {
            info: e.specifics.to_string(),
            hint: "".to_string(),
            span: FileID::default().with_range(e.position..e.position + 1),
        }
    }
}

impl From<ParseError> for NyarError {
    fn from(value: ParseError) -> Self {
        SyntaxError::from(value).into()
    }
}
