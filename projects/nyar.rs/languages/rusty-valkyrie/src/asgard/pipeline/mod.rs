//! Asgard 项目源码发现。
//!
//! 旧 `compile_voa_project` 应用管线编排已随文本转换配置链清退：
//! 配置求值（C0/C2）未闭合前，本模块只保留源码发现；
//! static 交付编排归 `static_site`，报告岛屿构建归 `report_build`。

mod discover;

pub use discover::{DiscoveredAwsFile, DiscoveredAwslFile, DiscoveredSources, discover_sources};
