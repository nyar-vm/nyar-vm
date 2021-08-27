use miette::{Severity, SourceCode};

use super::*;

#[derive(Debug)]
pub struct ParseError {
    /// The message to display to the user
    pub message: String,
    /// The path of the file relative to the workspace
    pub file: NamedSource,
    /// Where the error occurred
    pub span: Range<usize>,
}

impl Error for ParseError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Failed to parse the source")
    }
}

impl Diagnostic for ParseError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.file)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let label = vec![LabeledSpan::new(Some(self.message.clone()), self.span.start, self.span.end)];
        Some(box label.into_iter())
    }
}

impl From<ParseError> for NyarError {
    fn from(e: ParseError) -> Self {
        NyarError::ParseError(box e)
    }
}

impl From<ParseIntError> for NyarError3 {
    fn from(e: ParseIntError) -> Self {
        NyarError3::syntax_error(e.to_string())
    }
}

impl From<ParseCharError> for NyarError3 {
    fn from(e: ParseCharError) -> Self {
        NyarError3::syntax_error(e.to_string())
    }
}

impl From<ParseFloatError> for NyarError3 {
    fn from(e: ParseFloatError) -> Self {
        Self { kind: box NyarErrorKind::ParseDecimalError(e), span: Default::default() }
    }
}
