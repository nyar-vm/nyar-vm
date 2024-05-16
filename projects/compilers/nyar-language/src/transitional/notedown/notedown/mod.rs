#![doc = include_str!("readme.md")]

mod error;
mod lexer;
mod parser;
mod syntax;

pub mod formatter;

pub use error::NotedownError;
pub use lexer::{NotedownLexer, NotedownToken, NotedownTokenKind, parse_attr};
pub use parser::{parse, parse_inlines};
pub use syntax::*;

impl NotedownDocument {
    /// 解析 Notedown 源文本。
    pub fn parse(source: &str) -> Self {
        parser::parse(source)
    }
}
