#![feature(once_cell)]
#![feature(generators)]
#![feature(box_syntax)]
#![feature(try_blocks)]

extern crate core;

pub use self::grammar::context::ParsingContext;
pub use crate::grammar::parser::Rule;
pub use nyar_error::Result;
pub use traits::ASTDump;

mod grammar;
pub mod utils;

pub mod ast {
    pub use nyar_hir::ast::*;
}

mod traits;
