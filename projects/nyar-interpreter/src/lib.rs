#![feature(arbitrary_self_types)]
#![feature(once_cell)]
#![feature(box_syntax)]

pub use self::value::Value;
pub use nyar_error::{NyarError3, Result};
pub use nyar_hir::{ASTKind, ASTNode};

pub mod engine;
mod evaluate;
mod symbols;
pub mod typing;
pub mod utils;
pub mod value;

pub use self::symbols::SymbolColor;
