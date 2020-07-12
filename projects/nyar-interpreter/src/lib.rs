#![feature(once_cell)]

pub mod error;
mod function;
mod gc;
mod value;

pub use error::{NyarError, Result};
