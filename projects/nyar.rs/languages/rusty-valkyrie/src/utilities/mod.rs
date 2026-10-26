//! Cross-cutting helpers with no product decisions.
//!
//! 错误模型见 [`readme`](readme.md)：诊断 = boost；操作 = 领域 enum。

#![doc = include_str!("readme.md")]

pub mod diagnostic;
#[macro_use]
pub mod report;
pub mod text;

pub use report::{UserMessage, display_failure, report_msg};
