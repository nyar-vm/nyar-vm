use crate::{NyarError, SyntaxError};
use diagnostic::{ReportKind, SourceID};
use std::ops::Range;

use yggdrasil_rt::{errors::YggdrasilErrorKind, YggdrasilError, YggdrasilRule};

impl<R: YggdrasilRule> From<YggdrasilError<R>> for SyntaxError {
    fn from(error: YggdrasilError<R>) -> Self {
        let range = Range { start: error.location.start as u32, end: error.location.end as u32 };

        match error.variant {
            YggdrasilErrorKind::InvalidRule { positives, negatives } => {
                let info = match negatives.as_slice() {
                    [] => format!("Lexer interrupted, unexpected end of stream."),
                    [s @ ..] => format!("Lexer interrupt, unexpected {:?}.", s),
                };
                Self { info, hint: format!("Except {:?}", positives), span: SourceID::default().with_range(range) }
            }
            YggdrasilErrorKind::InvalidNode { .. } => {
                Self { info: error.variant.to_string(), hint: "".to_string(), span: SourceID::default().with_range(range) }
            }
            YggdrasilErrorKind::InvalidTag { .. } => {
                Self { info: error.variant.to_string(), hint: "".to_string(), span: SourceID::default().with_range(range) }
            }
            YggdrasilErrorKind::CustomError { .. } => {
                Self { info: error.variant.to_string(), hint: "".to_string(), span: SourceID::default().with_range(range) }
            }
        }
    }
}

impl<R: YggdrasilRule> From<YggdrasilError<R>> for NyarError {
    fn from(error: YggdrasilError<R>) -> Self {
        SyntaxError::from(error).as_error(ReportKind::Error)
    }
}
