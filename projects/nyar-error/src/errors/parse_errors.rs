use super::*;
use crate::NyarResult;
use std::fs::read_to_string;

#[derive(Debug)]
pub struct ParseError {
    /// The message to display to the user
    pub message: String,
    /// The path of the file relative to the workspace
    pub file: String,
    /// Absolute path
    pub url: Url,
    /// Where the error occurred
    pub span: Range<usize>,
}

impl Error for ParseError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to parse because")
    }
}

impl ParseError {
    pub fn label(&self) -> Vec<LabeledSpan> {
        vec![LabeledSpan::new(Some(self.message.clone()), self.span.start, self.span.end)]
    }
    pub fn text(&self) -> NyarResult<String> {
        read_to_string(self.url.to_file_path()?)?
    }
}

impl From<ParseError> for NyarError2 {
    fn from(e: ParseError) -> Self {
        NyarError2::ParseError(box e)
    }
}

impl From<ParseIntError> for NyarError {
    fn from(e: ParseIntError) -> Self {
        NyarError::syntax_error(e.to_string())
    }
}

impl From<ParseCharError> for NyarError {
    fn from(e: ParseCharError) -> Self {
        NyarError::syntax_error(e.to_string())
    }
}

impl From<ParseFloatError> for NyarError {
    fn from(e: ParseFloatError) -> Self {
        Self { kind: box NyarErrorKind::ParseDecimalError(e), span: Default::default() }
    }
}
