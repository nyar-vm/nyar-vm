use peginator::ParseError;

use super::*;

impl From<ParseError> for SyntaxError {
    fn from(e: ParseError) -> Self {
        let offset = e.position as u32;
        SyntaxError {
            info: e.specifics.to_string(),
            hint: "".to_string(),
            span: FileID::default().with_range(offset..offset + 1),
        }
    }
}

impl From<ParseError> for NyarError {
    fn from(value: ParseError) -> Self {
        SyntaxError::from(value).into()
    }
}
