//! 库导出面的唯一派生点。
//!
//! 导出面只来自**声明的导出面**：
//! - `TargetExecutionPlan::wasm_export_names`（`[export]` / `[test]` 冻结的公共名），
//! - `TargetExecutionPlan::entry_operation`（程序入口）。
//!
//! 片段的可达闭包（`TargetExecutionPlan::exported_operations` 之外的内部调用图）
//! **不是**导出面；它只描述模块内部 `Call` 可见性。wasm 与 OVM 两个后端的导出集合
//! 判定必须都从这里取，不允许任何后端按「可达即导出」自行重算——否则带函数类型
//! 形参的内部辅助函数会被要求过库边界，撞上「`Function` 不许过边界」而误报。

use nyar_types::ItemInstanceId;

use crate::target_execution_plan::TargetExecutionPlan;

/// 导出面的实例集合：声明导出名 ∪ 程序入口（顺序与 `wasm_export_names` 一致）。
pub(crate) fn export_surface(plan: &TargetExecutionPlan) -> Vec<ItemInstanceId> {
    let mut surface = plan.wasm_export_names().keys().copied().collect::<Vec<_>>();
    if let Some(entry) = plan.entry_operation() {
        if !surface.contains(&entry) {
            surface.push(entry);
        }
    }
    surface
}

/// 单个实例是否属于导出面。所有后端导出表入表判定都走这一个谓词。
pub(crate) fn contains(plan: &TargetExecutionPlan, operation: ItemInstanceId) -> bool {
    plan.wasm_export_names().contains_key(&operation) || plan.entry_operation() == Some(operation)
}
