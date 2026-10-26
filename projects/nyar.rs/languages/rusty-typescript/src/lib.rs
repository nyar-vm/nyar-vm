#![deny(missing_docs)]
//! Rusty TypeScript 语言前端
//!
//! 这个库提供了 Rusty TypeScript 语言的解析和 Nyar 翻译功能。
//! 遵循 Project Chomsky Whitebook 规范。

#![feature(new_range_api)]


pub mod codegen;
pub mod errors;
pub mod frontend;
pub(crate) mod frontend_emit;
pub(crate) mod frontend_parse;
pub mod project;
pub mod runtime;
pub mod type_system;

pub use crate::runtime::RustyTypeScriptRuntime;
pub use frontend::RustyTypescriptFrontend;
