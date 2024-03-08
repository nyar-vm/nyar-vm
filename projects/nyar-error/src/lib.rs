#![allow(clippy::wrong_self_convention)]
pub use crate::{
    duplicates::{DuplicateError, DuplicateKind},
    errors::{NyarError, NyarErrorKind, Result, Validation},
    parsing::{ForeignInterfaceError, SyntaxError},
    runtime::RuntimeError,
};
pub mod third_party;
pub use crate::undefined::MissingError;
pub use diagnostic::{Diagnostic, ReportKind, SourceCache, SourceID, SourceSpan};
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
