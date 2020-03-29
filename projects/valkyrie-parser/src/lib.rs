#![feature(format_args_nl)]
extern crate pest;
#[macro_use]
extern crate lazy_static;

pub mod grammar;
pub mod pest_parser;
pub mod utils;

pub use grammar::get_ast;
