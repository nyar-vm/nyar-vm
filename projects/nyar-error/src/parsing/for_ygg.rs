use crate::{NyarError, SyntaxError};
use diagnostic::{FileID, ReportKind};
use yggdrasil_rt::{errors::YggdrasilErrorKind, YggdrasilError, YggdrasilRule};

impl<R: YggdrasilRule> From<YggdrasilError<R>> for SyntaxError {
    fn from(error: YggdrasilError<R>) -> Self {
        match error.variant {
            YggdrasilErrorKind::InvalidRule { positives, negatives } => {
                let info = match negatives.as_slice() {
                    [] => format!("Lexer interrupted, unexpected end of stream."),
                    [s @ ..] => format!("Lexer interrupt, unexpected {:?}.", s),
                };
                Self { info, hint: format!("Except {:?}", positives), span: FileID::default().with_range(error.location) }
            }
            YggdrasilErrorKind::InvalidNode { .. } => Self {
                info: error.variant.to_string(),
                hint: "".to_string(),
                span: FileID::default().with_range(error.location),
            },
            YggdrasilErrorKind::InvalidTag { .. } => Self {
                info: error.variant.to_string(),
                hint: "".to_string(),
                span: FileID::default().with_range(error.location),
            },
            YggdrasilErrorKind::CustomError { .. } => Self {
                info: error.variant.to_string(),
                hint: "".to_string(),
                span: FileID::default().with_range(error.location),
            },
        }
    }
}

impl<R: YggdrasilRule> From<YggdrasilError<R>> for NyarError {
    fn from(error: YggdrasilError<R>) -> Self {
        SyntaxError::from(error).as_error(ReportKind::Error)
    }
}
