#![feature(arbitrary_self_types)]
#![feature(once_cell)]
#![feature(box_syntax)]

pub mod effects;
pub mod error;
pub mod function;
mod gc;
pub mod module;
pub mod typing;
pub mod utils;
pub mod value;
pub mod variable;

pub use error::{NyarError, Result};
pub use value::Value;
