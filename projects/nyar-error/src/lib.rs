#![feature(box_syntax)]
#![feature(trivial_bounds)]

mod errors;
mod span;

pub use self::{
    errors::{NyarError, NyarErrorKind, Result},
    span::Span,
};
