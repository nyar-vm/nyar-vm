#![feature(once_cell)]

pub use grammar::ParsingContext;
pub use nyar_error::Result;
pub use traits::ASTDump;

pub mod grammar;
mod lexer;
pub mod utils;

pub mod ast {
    pub use nyar_hir::ast::*;
}

mod traits;
