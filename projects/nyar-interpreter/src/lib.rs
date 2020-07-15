#![feature(once_cell)]

pub mod effects;
pub mod error;
pub mod function;
mod gc;
pub mod typing;
pub mod value;

pub use error::{NyarError, Result};
pub use value::Value;
