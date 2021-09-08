#[cfg(feature = "num")]
pub use num::BigInt;
pub use url::Url;

pub use crate::{
    duplicates::DuplicateError,
    errors::{ValkyrieError, ValkyrieErrorKind, ValkyrieReport, ValkyrieResult},
    managers::{
        list::{FileID, FileSpan},
        TextManager,
    },
    parsing::SyntaxError,
    runtime::RuntimeError,
};

mod errors;

mod duplicates;
mod managers;
mod parsing;
mod runtime;
#[cfg(test)]
pub mod testing;
