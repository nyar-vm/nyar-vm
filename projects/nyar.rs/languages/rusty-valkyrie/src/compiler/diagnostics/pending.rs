//! 编译单元内可恢复诊断的暂存汇（diagnostic sink）。
//!
//! 各检查阶段在发现错误时，先把结构化诊断推入本汇，再按既有控制流返回首个错误：
//! 编译仍然在阶段边界 fail-closed，但同一编译单元内**全部**可独立判定的诊断都被保留下来，
//! 供 CLI / 测试在失败路径上一次取出并渲染成多诊断报告。
//!
//! 汇是线程局部的：编译管线与既有 `source_context` 线程局部源码上下文在同一线程内执行，
//! 不引入跨线程共享状态。每个编译单元入口（如 `compile_source_groups_to_artifacts`）
//! 负责在开始前 [`clear_pending_diagnostics`]，失败路径调用方用 [`take_pending_diagnostics`] 取走全部诊断。

use std::cell::RefCell;

use nyar_types::{DiagnosticRow, DiagnosticSeverity};

/// 暂存汇的硬上限：超出后不再暂存新诊断，只累计丢弃数量。
///
/// 渲染层另有更小的展示上限（见 `report::DiagnosticRenderOptions`），
/// 本上限只防止病态输入下单次编译的暂存内存无界增长。
pub const PENDING_DIAGNOSTIC_LIMIT: usize = 1024;

/// 单条已收集的编译诊断。
///
/// `row` 是跨阶段唯一结构化载体（`nyar_types::DiagnosticRow`），
/// `message` 是生产侧给出的人类可读主消息（不含定位后缀；
/// 定位信息由 `row.source` + `row.primary_span` 在渲染层展开为行列与代码片段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileDiagnostic {
    /// 结构化诊断行（code / phase / owner / source / span / args）。
    pub row: DiagnosticRow,
    /// 人类可读主消息（rustc 风格，定位由渲染层补充，不在此重复）。
    pub message: String,
}

impl CompileDiagnostic {
    /// 构造一条编译诊断。
    pub fn new(row: DiagnosticRow, message: impl Into<String>) -> Self {
        Self { row, message: message.into() }
    }
}

/// 一次编译收集到的全部诊断及丢弃计数。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingDiagnostics {
    /// 已暂存的诊断（至多 [`PENDING_DIAGNOSTIC_LIMIT`] 条）。
    pub diagnostics: Vec<CompileDiagnostic>,
    /// 超出暂存上限被丢弃的诊断数量。
    pub overflow: usize,
}

impl PendingDiagnostics {
    /// 是否没有任何诊断（含已丢弃的）。
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty() && self.overflow == 0
    }

    /// 诊断总数（暂存的 + 被丢弃的）。
    pub fn total(&self) -> usize {
        self.diagnostics.len() + self.overflow
    }
}

thread_local! {
    static PENDING: RefCell<PendingDiagnostics> = RefCell::new(PendingDiagnostics::default());
}

/// 清空当前线程的暂存汇；编译单元入口在开始编译前调用。
pub fn clear_pending_diagnostics() {
    PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        pending.diagnostics.clear();
        pending.overflow = 0;
    });
}

/// 向暂存汇推入一条诊断；超过硬上限时只累计丢弃数。
#[doc(hidden)]
pub fn push_compile_diagnostic(diagnostic: CompileDiagnostic) {
    PENDING.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.diagnostics.len() < PENDING_DIAGNOSTIC_LIMIT {
            pending.diagnostics.push(diagnostic);
        }
        else {
            pending.overflow += 1;
        }
    });
}

/// 取走当前线程暂存汇中的全部诊断（汇被重置为空）。
pub fn take_pending_diagnostics() -> PendingDiagnostics {
    PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()))
}

/// 当前线程暂存汇是否为空（不取走诊断）。
pub fn pending_diagnostics_is_empty() -> bool {
    PENDING.with(|pending| pending.borrow().is_empty())
}

/// 阶段边界收口判据：暂存汇中 Error 级诊断的总数与首条副本（不取走诊断）。
///
/// 超出硬上限被丢弃的诊断级别不可知，按 Error 保守计入；Warning/Info/Hint 不参与计数。
pub fn pending_error_summary() -> (usize, Option<CompileDiagnostic>) {
    PENDING.with(|pending| {
        let pending = pending.borrow();
        let first = pending.diagnostics.iter().find(|diagnostic| matches!(diagnostic.row.severity, DiagnosticSeverity::Error)).cloned();
        let recorded = pending.diagnostics.iter().filter(|diagnostic| matches!(diagnostic.row.severity, DiagnosticSeverity::Error)).count();
        (recorded + pending.overflow, first)
    })
}
