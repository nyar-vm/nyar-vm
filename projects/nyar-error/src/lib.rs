#![allow(clippy::wrong_self_convention)]
pub use crate::{
    duplicates::DuplicateError,
    errors::{NyarError, NyarErrorKind, Result, Validation},
    parsing::SyntaxError,
    runtime::RuntimeError,
};
pub mod third_party;
pub use crate::undefined::MissingError;
pub use diagnostic::{Diagnostic, FileCache, FileID, FileSpan, ReportKind};
pub use validatus::{
    Validate,
    Validation::{Failure, Success},
};
mod errors;

mod duplicates;
mod parsing;
mod runtime;
#[cfg(test)]
pub mod testing;
mod undefined;
