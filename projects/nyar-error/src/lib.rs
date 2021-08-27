#![feature(box_syntax)]
#![feature(trivial_bounds)]

pub use miette::{NamedSource, Report};

pub use self::{
    errors::{parse_errors::ParseError, NyarError, NyarErrorKind, Result},
    span::Span,
};

mod errors;
mod levels;
mod span;
pub mod third_party;

pub type NyarResult<T = ()> = std::result::Result<T, NyarError2>;
pub type ReportResult<T = ()> = std::result::Result<T, Report>;

pub enum NyarError2 {
    InternalError(Box<InternalError>),
    IoError(Box<std::io::Error>),
    ParseError(Box<ParseError>),
}
