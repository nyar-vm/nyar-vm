//! 结构化诊断（API / N-API / CLI 渲染共用）。

use serde::Serialize;

/// 源码中的一段区间（API 层合同，不依赖 compiler 内部类型）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegionSpan {
    /// 起始字节偏移（0-based，包含）。
    pub start: usize,
    /// 结束字节偏移（0-based，不包含）。
    pub end: usize,
    /// 起始行号（1-based，可选；无文件上下文时为空）。
    pub line: Option<u32>,
    /// 起始列号（1-based，可选）。
    pub column: Option<u32>,
}

impl LegionSpan {
    /// 仅字节偏移的 span（行列号由调用方在能拿到源码时再填）。
    pub fn byte_range(start: usize, end: usize) -> Self {
        Self { start, end, line: None, column: None }
    }
}

/// 编译或规划阶段标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LegionStage {
    /// Workspace 解析与 build plan。
    Plan,
    /// 源码快照与语义闭包输入。
    SourceSnapshot,
    /// Valkyrie → HIR 解析。
    Parse,
    /// HIR lowering / closure 检查。
    Hir,
    /// HIR → MIR 降低。
    Mir,
    /// Canonical IR 生成与验证。
    Canonical,
    /// 目标后端 emit。
    Emit,
    /// 聚合兜底：尚未细分到上述子阶段的编译期诊断仍归这里。
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
    /// 源码中的区间（若有）。
    pub span: Option<LegionSpan>,
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
            span: None,
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
            span: None,
            owner: "legion::compile".into(),
        }
    }
}
