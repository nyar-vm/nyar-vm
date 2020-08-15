#![feature(once_cell)]

pub mod grammar;
pub mod utils;

pub mod ast {
    pub use nyar_hir::ast::*;
}

mod traits;

pub use grammar::LexerContext;
pub use nyar_hir::{NyarError, NyarErrorKind, Result};
pub use traits::ASTDump;
