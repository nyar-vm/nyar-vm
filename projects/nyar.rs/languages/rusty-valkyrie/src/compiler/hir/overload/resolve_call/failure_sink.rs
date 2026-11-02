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
