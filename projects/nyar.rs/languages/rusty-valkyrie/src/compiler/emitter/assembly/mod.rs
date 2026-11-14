//! 从唯一 `CompiledValkyrieProgram` 选择分区并交给目标适配器。
//!
//! 分区选择不重新生产语义事实；Valkyrie 通过语言 × 目标适配器准备目标输入，
//! 编排层只核对分区身份，不要求任何公共 MIR 作为成功载荷。
use std::sync::Arc;

use nyar::{ArtifactPartition, ArtifactPartitionPlan, CanonicalTarget, ClrSuspendStrategy, Identifier, PlanningError, projection_policy_for_target_profile};
use nyar_emitter::{
    AdaptedFragmentInput, AssembledFragment, FragmentSubmission, LoweredBackendInput, TargetExecutionPlan, TargetInputRequest,
};

use super::pipeline::target_plan;
use crate::compiler::valkyrie_mir::CompiledValkyrieProgram;

/// 返回已解析的导出/入口数量；装配器不直接读取语义计划。
#[doc(hidden)]
pub fn build_output_surface_counts(compiled_program: &CompiledValkyrieProgram) -> (usize, usize) {
    let linked = &compiled_program.mir_program().linked;
    (linked.exports.len(), linked.entries.len())
}

/// 根据完整程序、目标与 projection 合同规划产物分区。
#[doc(hidden)]
pub fn plan_artifacts_from_compiled_program(
    compiled_program: &CompiledValkyrieProgram,
    target: CanonicalTarget,
    clr_suspend_strategy: ClrSuspendStrategy,
) -> std::result::Result<ArtifactPartitionPlan, PlanningError> {
    let target_profile = target.to_profile(None);
    let projection_policy = projection_policy_for_target_profile(&target_profile).map_err(|error| PlanningError::SemanticContract {
        module: compiled_program.mir_program().linked.module_name.clone(),
        stage: nyar_types::CompileStage::RepresentationPlan,
        detail: format!("目标 projection 合同失败: {error:?}"),
    })?;
    let linked = &compiled_program.mir_program().linked;
    let backend_registry = nyar_emitter::bundled_backend_registry_from_canonical(&linked.fragments, &target_profile, &projection_policy);
    ArtifactPartitionPlan::from_linked_program(
        linked,
        target,
        nyar::RewriteTheory::default(),
        projection_policy,
        backend_registry,
        clr_suspend_strategy,
    )
}

/// Valkyrie × 目标适配器：把已验证的 Valkyrie 程序映射为平台输入。
struct ValkyrieTargetAdapter {
    partition: ArtifactPartition,
    plan: Arc<TargetExecutionPlan>,
}

impl AdaptedFragmentInput for ValkyrieTargetAdapter {
    fn partition(&self) -> &ArtifactPartition {
        &self.partition
    }

    fn prepare(&self, request: &TargetInputRequest<'_>) -> nyar_emitter::Result<LoweredBackendInput> {
        let submission = FragmentSubmission::from_backend_plan((*self.plan).clone());
        LoweredBackendInput::from_fragment_submission(
            &submission,
            request.backend_family,
            request.host_boundary,
            request.output_dir,
            request.host_flavor,
            request.wasm_package_kind,
        )
    }
}

/// 从指定分区装配目标载荷，拒绝任何与链接身份不一致的计划。
#[doc(hidden)]
pub fn assemble_fragment(
    compiled_program: &CompiledValkyrieProgram,
    plan: &ArtifactPartitionPlan,
    partition_index: usize,
) -> std::result::Result<AssembledFragment, String> {
    if plan.module_name.to_string() != compiled_program.mir_program().linked.module_name {
        return Err(report!("前端计划与分区计划不匹配"));
    }
    if partition_index >= plan.partitions.len() {
        return Err(report!("分区索引 `{partition_index}` 超出范围"));
    }
    let partition = &plan.partitions[partition_index];
    let fragment_view = plan
        .fragment_views
        .iter()
        .find(|view| view.fragment_id == partition.fragment)
        .ok_or_else(|| report!("分区 `{}` 缺少优化后的语义片段视图", partition.name))?;
    let linked = &compiled_program.mir_program().linked;
    let fragment = linked.fragments.get(&partition.fragment).ok_or_else(|| report!("分区 `{}` 对应的语义片段不存在", partition.name))?;
    if partition.exported_operations != fragment.exported_operations
        || partition.entry_operation != fragment.entry_operation
        || fragment_view.canonical_operations != fragment.exported_operations
    {
        return Err(report!("分区 `{}` 的 callable 身份与片段合同不一致", partition.name));
    }

    let fragment_requires_suspend = fragment.required_capabilities.iter().any(|capability| capability.as_str() == "suspend");
    if fragment_requires_suspend {
        return Err(report!("分区 `{}` 要求 suspend，但 Compiler 尚未提供已验证的 suspend 合同", partition.name));
    }

    let mut roots = partition.exported_operations.clone();
    if let Some(entry) = partition.entry_operation
        && !roots.contains(&entry)
    {
        roots.push(entry);
    }
    let theory_bundle = fragment_view.theory_bundle.clone();
    // tag 宽度是平台服务提供的事实；语言侧不再声明它。
    let sum_tag_width = nyar_emitter::sum_tag_width(plan.target.to_profile(None).backend_family);
    let backend_plan = target_plan::from_compiled_program(compiled_program, &partition.fragment, theory_bundle.clone(), &roots, sum_tag_width)
        .map_err(|error| format!("目标计划构建失败: {error}"))?;

    Ok(AssembledFragment {
        partition: partition.clone(),
        theory_bundle,
        input: Box::new(ValkyrieTargetAdapter { partition: partition.clone(), plan: Arc::new(backend_plan) }),
    })
}
