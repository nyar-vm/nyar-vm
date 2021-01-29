use pest::{
    error::{Error, ErrorVariant, InputLocation},
    RuleType,
};

use crate::{NyarError, NyarErrorKind, Span};

impl<R> From<Error<R>> for NyarError
where
    R: RuleType,
{
    fn from(e: Error<R>) -> Self {
        let span = match e.location {
            InputLocation::Pos(s) => Span { start: 0, end: 0, file_id: 0 },
            InputLocation::Span((s, e)) => Span { start: 0, end: 0, file_id: 0 },
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
