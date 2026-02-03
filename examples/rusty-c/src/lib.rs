#![feature(new_range_api)]
//! Rusty C 解释器
//!
//! 基于 Oaks (前端), Chomsky (优化), Gaia (后端) 和 Nyar VM (运行时) 架构实现。

pub mod errors;
pub mod frontend;
pub mod optimizer;
pub mod runtime;

pub use crate::frontend::RustyCFrontend;
