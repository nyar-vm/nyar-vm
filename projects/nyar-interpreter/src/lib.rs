#![feature(arbitrary_self_types)]
#![feature(once_cell)]
#![feature(box_syntax)]
#![feature(arc_new_cyclic)]
#![feature(const_fn_trait_bound)]

pub use self::value::Value;
pub use nyar_error::{NyarError, Result};
pub use nyar_hir::{ASTKind, ASTNode};

pub mod engine;
pub mod typing;
pub mod utils;
pub mod value;
