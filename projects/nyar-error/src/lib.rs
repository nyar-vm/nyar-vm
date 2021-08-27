#![feature(box_syntax)]
#![feature(trivial_bounds)]
#![feature(type_name_of_val)]

pub use miette::{NamedSource, Report};
pub use url::Url;

use crate::errors::internal_error::InternalError;

pub use self::{
    errors::{parse_errors::ParseError, NyarError3, NyarErrorKind, Result},
    span::Span,
};

mod errors;
mod levels;
mod span;
pub mod third_party;

pub type NyarResult<T = ()> = std::result::Result<T, NyarError>;
pub type ReportResult<T = ()> = std::result::Result<T, Report>;

pub enum NyarError {
    IoError(Box<std::io::Error>),
    ParseError(Box<ParseError>),
    InternalError(Box<InternalError>),
}
