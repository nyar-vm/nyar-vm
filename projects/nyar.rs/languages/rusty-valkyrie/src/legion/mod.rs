//! Legion 工程编排门面。
//!
//! 实现已迁入 Boost 的 `compiler.valkyrie`（`compiler_valkyrie::legion`），本模块只做再导出，
//! 保证 `rusty_valkyrie::legion::…` 的既有路径不变。

pub use compiler_valkyrie::legion::*;

/// 为工程生成文档站点。
pub use compiler_valkyrie::legion::cmds::doc::generate_for_project;
