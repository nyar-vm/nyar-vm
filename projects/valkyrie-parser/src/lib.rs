#![feature(format_args_nl)]
extern crate pest;
#[macro_use]
extern crate lazy_static;

pub mod grammar;
pub mod pest_parser;
pub mod utils;

/// re-export
pub mod ast {
    pub use nyar_ast::{
        ast::{ImportStatement, Number},
        AST,
    };
}

pub use ast::AST;
pub use grammar::get_ast;
