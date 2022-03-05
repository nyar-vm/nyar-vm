pub use crate::{
    duplicates::DuplicateError,
    errors::{NyarError, NyarErrorKind, NyarResult},
    managers::list::FileSpan,
    parsing::SyntaxError,
    runtime::RuntimeError,
};

pub mod third_party {
    #[cfg(feature = "dashu")]
    pub use dashu::{
        float::{
            round::mode::{HalfAway, HalfEven},
            DBig, FBig,
        },
        integer::IBig,
    };
    #[cfg(feature = "num")]
    pub use num::BigInt;
    #[cfg(feature = "pratt")]
    pub use pratt::{Affix, Associativity, PrattParser, Precedence};
    pub use url::Url;
}
pub use diagnostic::{Diagnostic, FileCache};
mod errors;

mod duplicates;
mod managers;
mod parsing;
mod runtime;
#[cfg(test)]
pub mod testing;
