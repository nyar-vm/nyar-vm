#![deny(missing_docs)]

//! 编译期中立合同类型的消费侧薄封装。
//!
//! # 分层
//!
//! 语义身份、链接后程序事实、布局计划、可审计中立合同与诊断线格式的实现位于 Boost
//! **`compiler-types`**，本 crate 只做原样再导出，供语言前端、分析与后端消费。
//! 快照、字节访问与文本索引仍由 `boost-source` 拥有，诊断渲染与收集仍由 `boost-harness` 拥有。

pub use compiler_types::*;
