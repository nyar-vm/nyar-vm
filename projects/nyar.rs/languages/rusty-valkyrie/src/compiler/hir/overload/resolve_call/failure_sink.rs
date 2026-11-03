//! 调用解析失败的阶段级收集器。
//!
//! 表达式树深处的解析失败逐条登记到当前收集器；每个模块层级在解析结束后
//! 取走本层失败事实并落到该模块的 `call_resolution_failures` 注册表。
//! 线程局部存储与 `source_context` 的 lowering 上下文同构，按线程隔离。

use std::cell::RefCell;

use crate::compiler::types::hir::HirCallResolutionFailure;

thread_local! {
    static COLLECTOR_STACK: RefCell<Vec<Vec<HirCallResolutionFailure>>> = const { RefCell::new(Vec::new()) };
}

/// 压入一个新收集器执行闭包，返回闭包结果与该层登记的全部失败事实。
///
/// 嵌套调用各层独立：子模块解析产生的失败归子模块自己的收集器。
pub(crate) fn with_failure_collector<R>(f: impl FnOnce() -> R) -> (R, Vec<HirCallResolutionFailure>) {
    COLLECTOR_STACK.with(|stack| stack.borrow_mut().push(Vec::new()));
    let result = f();
    let collected = COLLECTOR_STACK.with(|stack| stack.borrow_mut().pop().unwrap_or_default());
    (result, collected)
}

/// 向当前收集器登记一条失败事实；没有活动收集器时丢弃（例如直接调用解析的探针）。
pub(crate) fn record_failure(failure: HirCallResolutionFailure) {
    COLLECTOR_STACK.with(|stack| {
        if let Some(top) = stack.borrow_mut().last_mut() {
            top.push(failure);
        }
    });
}

/// 用同一调用点的更精确失败替换普通失败。
///
/// 脱糖节点的义务判定发生在通用调用解析之后：普通解析可能已为同一个 span
/// 登记了「方法不存在」，而协议义务检查能给出更准确的失败类别。只替换同 span
/// 的既有登记，不吞掉源表达式内部其它调用点的失败。
pub(crate) fn replace_failure_at(span: &crate::compiler::types::SourceSpan, failure: HirCallResolutionFailure) {
    COLLECTOR_STACK.with(|stack| {
        if let Some(top) = stack.borrow_mut().last_mut() {
            top.retain(|existing| &existing.span != span);
            top.push(failure);
        }
    });
}

/// 该调用点是否已经登记了更精确的失败事实。
///
/// 供上层在通用「无一候选」兜底前查询，避免用宽泛失败覆盖已记录的结构化原因。
pub(crate) fn failure_recorded_at(span: &crate::compiler::types::SourceSpan) -> bool {
    COLLECTOR_STACK.with(|stack| stack.borrow().last().is_some_and(|top| top.iter().any(|existing| &existing.span == span)))
}
