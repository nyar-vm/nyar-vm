//! Oak 诊断边界辅助：在展示层之前保留 `source_offset`。

use std::ops::Range;

use crate::legion::cmds::error::LegionError;
use oak_core::OakError;

/// 将 Oak 单点偏移转为字节区间。
pub fn source_point_span(error: &OakError) -> Option<Range<usize>> {
    error.source_offset().map(|start| start..start.saturating_add(1))
}

/// 将 Oak 解析错误转为 Legion 错误（结构化标签见 `utilities::diagnostic` / boost）。
pub fn labeled_report(error: OakError, label: impl Into<String>) -> LegionError {
    LegionError::oak("", label, error)
}

/// 将 Oak 解析错误包装为带上下文的 Legion 错误。
pub fn labeled_report_with_context(error: OakError, context: impl std::fmt::Display, label: impl Into<String>) -> LegionError {
    LegionError::oak(context.to_string(), label, error)
}
