#![feature(box_syntax)]
#![feature(int_error_matching)]

pub mod ast;

mod errors;

pub use ast::{ASTKind, ASTNode};
pub use errors::{NyarError, NyarErrorKind, Result};
pub use lsp_types::{Position, Range};
