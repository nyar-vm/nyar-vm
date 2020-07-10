#![feature(format_args_nl)]
extern crate pest;
#[macro_use]
extern crate lazy_static;

pub mod grammar;
pub mod pest_parser;
pub mod utils;

/// re-export
pub mod ast {
    pub use nyar_ast::{ast::ImportStatement, AST};
}

pub use ast::AST;
pub use grammar::Settings;

pub fn get_ast(input: &str) -> AST {
    let mut cfg = Settings::default();
    cfg.refine = true;
    cfg.get_ast(input)
}
