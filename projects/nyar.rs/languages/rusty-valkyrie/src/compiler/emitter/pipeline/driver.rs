//! Compiler 在依赖闭包完成后生产唯一的已验证成功载荷。

use nyar_types::{CompileStage, CompiledProgram, StageResult, pipeline::RepresentationPlanStage};

use super::{CanonicalRepresentationPlanner, canonical_program_from_semantic_mir, fail_stage};
use crate::compiler::mir::{MirModule, validation::validate_semantic_module};

/// 校验 Semantic MIR 并生产唯一的已验证成功载荷；集成测试夹具。
#[doc(hidden)]
pub fn compile_linked_semantic_mir(module: &MirModule) -> StageResult<CompiledProgram> {
    if let Err(error) = validate_semantic_module(module) {
        // 诊断沿用合同自身的机器码与结构化字段，下游报告才能按真实 SMIR 码区分失败。
        let message = format!("Semantic MIR 合同失败: function={}, location={}, detail={}", error.function, error.location, error.detail);
        return fail_stage(CompileStage::ValidateMir, error.code, &module.name, message);
    }
    let canonical = canonical_program_from_semantic_mir(module)?;
    let representation = CanonicalRepresentationPlanner.plan(&canonical)?;
    match CompiledProgram::new(canonical, representation) {
        Ok(program) => Ok(program),
        // 合同错误的渲染归 `CompiledProgramError` 自身所有，管线只做前缀与诊断码映射。
        Err(error) => fail_stage(
            CompileStage::RepresentationPlan,
            "PIPE005",
            &module.name,
            format!("CanonicalProgram 与 RepresentationPlan 合同不一致: {error}"),
        ),
    }
}
