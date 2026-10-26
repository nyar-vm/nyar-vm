//! 结构化诊断（API / N-API / CLI 渲染共用）。

use serde::Serialize;

/// 编译或规划阶段标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LegionStage {
    /// Workspace 解析与 build plan。
    Plan,
    /// 源码快照与语义闭包输入。
    SourceSnapshot,
    /// HIR / MIR / Canonical / Emit 聚合阶段。
    Compile,
    /// 产物缓存读写。
    Cache,
    /// 产物落盘与 manifest。
    Artifact,
}

/// 严重级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LegionSeverity {
    /// 错误。
    Error,
    /// 警告。
    Warning,
    /// 附注。
    Note,
}

/// 单条结构化诊断。
#[derive(Debug, Clone, Serialize)]
pub struct LegionDiagnostic {
    /// 发生阶段。
    pub stage: LegionStage,
    /// 稳定错误码（机器可读）。
    pub code: String,
    /// 严重级别。
    pub severity: LegionSeverity,
    /// 用户可见说明。
    pub message: String,
    /// 相关源文件（若有）。
    pub source: Option<String>,
    /// 事实所有者（模块路径提示）。
    pub owner: String,
}

impl LegionDiagnostic {
    /// 规划阶段错误。
    pub fn plan_error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            stage: LegionStage::Plan,
            code: code.into(),
            severity: LegionSeverity::Error,
            message: message.into(),
            source: None,
            owner: "legion::workspace".into(),
        }
    }

    /// 编译阶段错误。
    pub fn compile_error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            stage: LegionStage::Compile,
            code: code.into(),
            severity: LegionSeverity::Error,
            message: message.into(),
            source: None,
            owner: "legion::compile".into(),
        }
    }
}
