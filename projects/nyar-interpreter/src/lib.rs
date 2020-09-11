#![feature(arbitrary_self_types)]
#![feature(once_cell)]
#![feature(box_syntax)]

pub use nyar_hir::{ASTKind, ASTNode};
pub use value::{
    error::{NyarError, Result},
    Value,
};

pub mod engine;
pub mod typing;
pub mod utils;
pub mod value;
