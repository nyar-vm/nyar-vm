use crate::ast::Stmt;
use crate::lexer::{lex, Error};
use crate::parser::parse;
use nyar_vm::bytecode::format::NyarcModule;

pub mod ast;
pub mod compiler;
pub mod lexer;
pub mod parser;

pub fn compile_text_to_module(src: &str) -> Result<NyarcModule, Error> {
    let toks = lex(src)?;
    let ast = parse(&toks)?;
    compiler::compile(&ast)
}
