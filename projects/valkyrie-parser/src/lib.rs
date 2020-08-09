#![feature(once_cell)]

pub mod grammar;
pub mod utils;
pub mod ast {
    pub use nyar_hir::ast::*;
}

mod errors;

pub use grammar::LexerContext;

pub fn get_ast(input: &str) -> nyar_hir::ASTKind {
    let mut cfg = LexerContext::default();
    cfg.refine = true;
    cfg.get_ast(input).unwrap()
}
