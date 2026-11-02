//! SSG 渲染的进程内 OVM 执行与读出。
//!
//! 加载 SSG 编译产物、调用 `awsl_ssg_render` 导出、从堆还原显式类型化的
//! DOM 树。加载/执行/读出任何一步失败都是结构化错误，不让进程崩掉。

use odin_vm::{OdinVm, dom_runtime::DomNode};

use crate::asgard::error::{AsgardError, AsgardResult};

/// SSG 渲染导出名（与发射器冻结一致）。
pub(crate) const SSG_RENDER_EXPORT: &str = "awsl_ssg_render";

/// 用 Legion 内置 Odin VM 进程内执行 SSG 渲染入口并读出 DOM 树。
pub(crate) fn run_ssg_render(artifact: &[u8]) -> AsgardResult<DomNode> {
    let mut vm = OdinVm::new();
    let loaded = vm.load(artifact).map_err(|error| AsgardError::msg(report!("OVM 加载 SSG 产物失败: {error}")))?;
    let root = vm
        .run(&loaded, SSG_RENDER_EXPORT, Vec::new())
        .map_err(|error| AsgardError::msg(report!("OVM 执行 `{SSG_RENDER_EXPORT}` 失败: {error}")))?;
    odin_vm::dom_runtime::read_tree(vm.heap(), &root).map_err(|error| AsgardError::msg(report!("SSG DOM 树读出失败: {error}")))
}
