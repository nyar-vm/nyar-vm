use std::ops::Range;

use super::*;

#[derive(Debug)]
pub struct ParseError {
    message: String,
    file: NamedSource,
    span: Range<usize>,
}

impl ParseError {
    pub fn new(message: String, source: NamedSource, position: Range<usize>) -> Self {
        Self { message, file: source, span: Range { start: position.start, end: position.end } }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to parse because")
    }
}

impl Error for ParseError {}

impl Diagnostic for ParseError {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(box "E9000")
    }
    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(box &self.message)
    }
    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(box format!("https://docs.rs/E9000"))
    }
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }
    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.file)
    }
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(box vec![LabeledSpan::new(Some(self.message.clone()), self.span.start, self.span.end)].into_iter())
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
