//! Legion 工程编排门面。
//!
//! 实现位于 Boost 的 `package.manager`（`package_manager::legion`），本模块只做再导出，
//! 保证 `rusty_valkyrie::legion::…` 的既有路径不变。

pub use nyar_package_manager::legion::*;

/// 为工程生成文档站点。
pub use nyar_package_manager::legion::cmds::doc::generate_for_project;