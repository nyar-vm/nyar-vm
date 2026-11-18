#![doc = include_str!("readme.md")]
#![deny(missing_docs)]

/// 前端无关的报告 / SSG / hydrate 岛契约。
pub mod testing;

/// 目标解析、产物规划与后端注册合同位于 Boost `compiler-target`。
pub use compiler_target::*;
