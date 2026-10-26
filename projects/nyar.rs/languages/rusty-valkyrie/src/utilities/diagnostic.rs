//! `boost-diagnostic` 结构化诊断（唯一展示/交换格式，替代 miette `Diagnostic`）。
//!
//! 操作类失败（无源码锚点）不要塞进本模块；见 [`readme`](readme.md)。

pub use boost_diagnostic::{Diagnostic, DiagnosticCode, DiagnosticSet, DiagnosticSeverity, DiagnosticSink, SinkStatus};

// `ValkyrieError` → `Diagnostic` 转换在 compiler/diagnostics 层按合同补齐。
