//! 前端无关的格式化契约（实现、注册表与 `Document`/CST/`SourceMap` 归 Oak `oak-formats`）。
//!
//! 保留本模块只为既有消费者路径（`nyar_analyzer::format::*`）不变；本仓不再持有实现。

pub use oak_formats::contract::*;
