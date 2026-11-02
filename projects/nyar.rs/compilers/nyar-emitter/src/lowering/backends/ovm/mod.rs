use odin_bytecode::{Odin_VERSION, OdinModuleData};

use super::sanitize_symbol;
use crate::FragmentSubmission;

/// 只发射 Compiler 实例键控的目标私有计划，不重放调用边（集成测试专用）。
#[doc(hidden)]
pub fn lower_fragment_to_nyar_module(submission: &FragmentSubmission) -> crate::Result<OdinModuleData> {
    if !submission.backend_plan.instances().is_empty() {
        return super::ovm_mir::lower_fragment_mir_to_nyar_module(submission);
    }
    Err(format!("Nyar VM backend requires Compiler-owned executable functions; edge-based semantic replay is not a valid input"))
}

/// 库模式公开导出名（run-contract 入口）；与模块内 `NyarExport.symbol_name` 一一对应（集成测试专用）。
#[doc(hidden)]
pub fn library_public_export_names(submission: &FragmentSubmission) -> Vec<String> {
    super::ovm_mir::nyar_library_public_export_names(submission)
}
