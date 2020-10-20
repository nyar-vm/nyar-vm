#![feature(arbitrary_self_types)]
#![feature(once_cell)]
#![feature(box_syntax)]
#![feature(arc_new_cyclic)]

pub use nyar_hir::{ASTKind, ASTNode, NyarError, Result};
pub use value::Value;

pub mod engine;
pub mod typing;
pub mod utils;
pub mod value;
