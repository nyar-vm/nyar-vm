#![doc = include_str!("readme.md")]
#![deny(missing_docs)]

extern crate self as rusty_valkyrie;

#[macro_use]
pub mod utilities;
pub mod asgard;
pub mod compiler;
pub mod legion;

/// Stable compiler façade (no `nyar` re-export at crate root).
pub mod api {
    pub use crate::compiler::api::*;
}
