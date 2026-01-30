//! Mini Rust 语言实现
#![feature(new_range_api)]

pub mod ast;
pub mod codegen;
pub mod converter;
// pub mod lexer;
// pub mod parser;

pub use codegen::MiniRustParser;
