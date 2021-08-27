#![feature(box_syntax)]
#![feature(trivial_bounds)]

pub use miette::{NamedSource, Result};

pub use self::{
    errors::{parse_errors::ParseError, NyarError, NyarErrorKind, NyarResult},
    levels::ErrorLevels,
    span::Span,
};

mod errors;
mod levels;
mod span;
pub mod third_party;
