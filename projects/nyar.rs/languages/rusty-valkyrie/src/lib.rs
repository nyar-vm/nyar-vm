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

/// 供外部集成测试引用的内部符号工具（不稳定，勿在正式代码中使用）。
#[doc(hidden)]
pub use compiler::emitter::backend_contract::interop::function_interop_contract;
/// 供外部集成测试引用的内部符号工具（不稳定，勿在正式代码中使用）。
#[doc(hidden)]
pub use compiler::hir::symbols::{stable_hir_function_name_path, stable_hir_function_symbol};
