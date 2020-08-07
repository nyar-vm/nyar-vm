#![feature(box_syntax)]


pub mod ast;

mod errors;

pub use ast::{ASTKind, ASTNode};
pub use errors::{Result, NyarError, NyarErrorKind};