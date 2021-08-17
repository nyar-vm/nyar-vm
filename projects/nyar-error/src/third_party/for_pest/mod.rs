use pest::{
    error::{Error, ErrorVariant, InputLocation},
    iterators::Pair,
    RuleType,
};

use crate::{NyarError, NyarErrorKind, Span};

impl<R> From<Error<R>> for NyarError
where
    R: RuleType,
{
    fn from(e: Error<R>) -> Self {
        let span = match e.location {
            InputLocation::Pos(s) => Span { start: s as u32, end: s as u32, file_id: 0 },
            InputLocation::Span((s, e)) => Span { start: s as u32, end: e as u32, file_id: 0 },
        };
        let info = match e.variant {
            ErrorVariant::ParsingError { positives, negatives } => {
                format!("expected: {:?}\nfound: {:?}", positives, negatives)
            }
            ErrorVariant::CustomError { message } => message,
        };
        NyarError { kind: box NyarErrorKind::SyntaxError { info }, span }
    }
}

impl Span {
    pub fn from_pair<R>(pair: &Pair<R>, file: u32) -> Self
    where
        R: RuleType,
    {
        let span = pair.as_span();
        Self { start: span.start() as u32, end: span.end() as u32, file_id: file }
    }
}
