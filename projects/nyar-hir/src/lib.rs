#![feature(box_syntax)]
#![feature(int_error_matching)]
#![feature(trivial_bounds)]

pub mod ast;

mod errors;

pub use ast::{ASTKind, ASTNode};
pub use errors::{NyarError, NyarErrorKind, Result};
