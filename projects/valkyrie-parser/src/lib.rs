#![feature(once_cell)]

pub mod grammar;
pub mod utils;

pub mod ast {
    pub use nyar_hir::ast::*;
}

mod traits;

pub use grammar::LexerContext;
pub use traits::ASTDump;
pub use nyar_hir::{Result, NyarError, NyarErrorKind};