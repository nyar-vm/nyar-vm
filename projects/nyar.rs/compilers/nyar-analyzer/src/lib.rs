#![doc = include_str!("readme.md")]
#![deny(missing_docs)]

/// 前端无关源码格式化 / printer 基础设施（平台层）。
pub mod format;
/// 前端无关语法高亮基础设施（平台层）。
pub mod highlight;
/// 前端无关报告 / SSG / hydrate 岛契约（平台层）。
pub mod report;
/// 前端无关 PE 残化接收端契约（平台层）。
pub mod residual;

/// 事实合同（入口、导入、导出、函数属性与名义类型定义）位于 Boost `compiler-analysis`。
pub use compiler_analysis::*;
/// 运行时需求标签仍由中立类型合同提供。
pub use compiler_types::RuntimeRequirement;
