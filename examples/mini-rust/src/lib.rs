//! Mini Rust 语言实现

pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;

pub use codegen::MiniRustParser;
