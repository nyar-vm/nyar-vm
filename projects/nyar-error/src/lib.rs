pub use crate::{
    duplicates::DuplicateError,
    errors::{NyarError, NyarErrorKind, NyarResult},
    parsing::SyntaxError,
    runtime::RuntimeError,
};

pub mod third_party;

pub use diagnostic::{Diagnostic, FileCache, FileSpan};
mod errors;

mod duplicates;
mod parsing;
mod runtime;
#[cfg(test)]
pub mod testing;
