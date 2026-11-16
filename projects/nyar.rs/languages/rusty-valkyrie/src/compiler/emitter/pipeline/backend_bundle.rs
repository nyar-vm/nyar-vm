//! Compiler 生成的完整目标编译 bundle。
//!
//! Legion 与其他装配层只能提交源码快照和目标合同；分区规划、fragment
//! assembly 以及目标私有输入都在 Compiler 内完成。

use super::error::{PipelineError, PipelineResult};
use nyar::{ArtifactPartitionPlan, TargetBackendFamily};
use nyar_emitter::{FrontendBuildBundle, PlannedArtifactPartitionsView};
use std::path::Path;

use crate::compiler::{
    emitter::assembly::{assemble_fragment, build_output_surface_counts, plan_artifacts_from_compiled_program},
    hir::lowering::{CompilerSourceGroup, CompilerSourceInput, ValkyrieCompiler},
};

use super::{context::CompilerBuildContext, evidence::CompilerCompileEvidence};

/// Wasm artifact surface 合同校验失败（结构化，不包装成 Display 文本）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtifactSurfaceError {
    /// `artifact: library` 要求至少一个已解析的 export。
    LibraryMissingExport,
    /// `artifact: binary` 要求一个已解析的 main entry。
    BinaryMissingEntry,
}

impl std::fmt::Display for ArtifactSurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LibraryMissingExport => f.write_str("`artifact: library` requires at least one resolved export"),
            Self::BinaryMissingEntry => f.write_str("`artifact: binary` requires a resolved main entry"),
        }
    }
}

impl std::error::Error for ArtifactSurfaceError {}

/// 完整目标产物与编译证据。
#[derive(Debug)]
pub struct CompilerArtifactReport {
    /// 驱动层产物报告。
    pub driver: nyar_emitter::DriverCompileReport,
    /// 可复现的编译证据。
    pub evidence: CompilerCompileEvidence,
}

/// Compiler 已完成语义分析、表示规划和分区装配的目标输入 bundle。
#[doc(hidden)]
pub struct CompilerBuildBundle {
    compiled_program: crate::compiler::valkyrie_mir::CompiledValkyrieProgram,
    artifact_plan: ArtifactPartitionPlan,
    artifact_purpose: nyar::ArtifactPurpose,
    wasm_package_kind: nyar_emitter::nyar_backend_wasi::WasmPackageKind,
    host_contract: Option<nyar::ArtifactContract>,
    test_entry_names: Vec<String>,
}

impl CompilerBuildBundle {
    /// 返回已由 Canonical 成功载荷确认的导出和入口数量。
    #[doc(hidden)]
    pub fn surface_counts(&self) -> (usize, usize) {
        build_output_surface_counts(&self.compiled_program)
    }
}

/// 从 Resolver 提供的源码组生产唯一目标 bundle；外部测试入口。
#[doc(hidden)]
pub fn compile_source_groups_to_backend_bundle(
    compiler: &ValkyrieCompiler,
    groups: &[CompilerSourceGroup],
    context: &CompilerBuildContext,
) -> PipelineResult<CompilerBuildBundle> {
    // 编译单元入口清空诊断暂存汇：本单元内各检查阶段记录的可恢复诊断
    // 都属于本单元，失败路径由调用方一次性取出渲染，不带上一单元的残留。
    crate::compiler::diagnostics::pending::clear_pending_diagnostics();
    let compiled_program = compiler.compile_source_groups_to_program_with_host_bindings(
        groups,
        &context.selected_host_providers,
        &context.gated_out_host_providers,
        &context.arch,
    )?;
    // 产物化前的最终边界：任何阶段遗留在暂存汇中的 Error 级诊断都必须在产物落盘前收口，
    // 不允许带被丢弃项的编译单元产出产物。
    crate::compiler::hir::lowering::HirError::reject_pending_errors()?;
    let artifact_plan = plan_artifacts_from_compiled_program(&compiled_program, context.target.clone(), context.clr_suspend_strategy)?;
    let bundle = CompilerBuildBundle {
        compiled_program,
        artifact_plan,
        artifact_purpose: context.artifact_purpose,
        wasm_package_kind: context.wasm_package_kind,
        host_contract: context.host_contract.clone(),
        test_entry_names: context.test_entry_names.clone(),
    };
    validate_artifact_surface(&bundle)?;
    Ok(bundle)
}

/// 从完整源码输入（应用组 + 独立生成组）生产目标产物报告。
///
/// 生成源码在这里合成到唯一源码闭包；不存在绕过生成组证据的产物路径。
pub fn compile_source_groups_to_artifacts(
    compiler: &ValkyrieCompiler,
    input: &CompilerSourceInput,
    context: &CompilerBuildContext,
    output_dir: &Path,
    project_name: &str,
    emit_wat_sidecar: bool,
    generate_runtime_config: bool,
) -> PipelineResult<CompilerArtifactReport> {
    let groups = crate::compiler::hir::lowering::compose_source_input_groups(input)?;
    let bundle = compile_source_groups_to_backend_bundle(compiler, &groups, context)?;
    let evidence = CompilerCompileEvidence::from_success_with_generated(
        &groups,
        &input.generated,
        &context.arch,
        &bundle.compiled_program,
        &context.selected_host_providers,
    );
    let driver = nyar_emitter::compile_frontend_bundle_with_bundled_backends(
        &bundle,
        output_dir,
        project_name,
        emit_wat_sidecar,
        generate_runtime_config,
    )?;
    Ok(CompilerArtifactReport { driver, evidence })
}

fn validate_artifact_surface(bundle: &CompilerBuildBundle) -> PipelineResult<()> {
    let (export_count, entry_count) = bundle.surface_counts();
    // 「library 必须有导出面」是全 lane 合同：OVM / JVM / CLR 不得回落 `main`
    // 或静默产出空运行契约。
    if bundle.artifact_purpose == nyar::ArtifactPurpose::Library && export_count == 0 {
        return Err(PipelineError::Surface(ArtifactSurfaceError::LibraryMissingExport));
    }
    // 宿主扩展的交付面就是冻结导出面：空导出面没有可装载的宿主 API。
    if bundle.artifact_purpose == nyar::ArtifactPurpose::HostExtension && export_count == 0 {
        return Err(PipelineError::Surface(ArtifactSurfaceError::LibraryMissingExport));
    }
    // 测试包必须至少有一个 `[test]` 入口身份。
    if bundle.artifact_purpose == nyar::ArtifactPurpose::TestBundle && bundle.test_entry_names.is_empty() {
        return Err(PipelineError::Surface(ArtifactSurfaceError::BinaryMissingEntry));
    }
    // 应用入口检查仍只约束 wasm（其余 lane 的 host 入口由各自后端按用途冻结）。
    if bundle.artifact_purpose == nyar::ArtifactPurpose::Application
        && bundle.artifact_plan.target.to_profile(None).backend_family == TargetBackendFamily::Wasm
        && bundle.wasm_package_kind == nyar_emitter::nyar_backend_wasi::WasmPackageKind::Binary
        && entry_count == 0
    {
        return Err(PipelineError::Surface(ArtifactSurfaceError::BinaryMissingEntry));
    }
    Ok(())
}

impl FrontendBuildBundle for CompilerBuildBundle {
    fn planned_partitions(&self) -> &dyn PlannedArtifactPartitionsView {
        self
    }

    fn artifact_purpose(&self) -> nyar::ArtifactPurpose {
        self.artifact_purpose
    }

    fn wasm_package_kind(&self) -> nyar_emitter::nyar_backend_wasi::WasmPackageKind {
        self.wasm_package_kind
    }

    fn host_contract(&self) -> Option<nyar::ArtifactContract> {
        self.host_contract.clone()
    }

    fn test_entry_names(&self) -> Vec<String> {
        self.test_entry_names.clone()
    }

    fn assemble_fragment_for_partition(
        &self,
        partition_index: usize,
    ) -> std::result::Result<nyar_emitter::AssembledFragment, nyar_emitter::EmitterError> {
        assemble_fragment(&self.compiled_program, &self.artifact_plan, partition_index).map_err(|error| error.to_string())
    }

    fn target_host_flavor(&self) -> Option<String> {
        Some(self.artifact_plan.target.to_profile(None).host_flavor)
    }
}

impl PlannedArtifactPartitionsView for CompilerBuildBundle {
    fn primary_partition_name(&self) -> Option<String> {
        self.artifact_plan
            .partitions
            .iter()
            .find(|partition| partition.entry_operation.is_some())
            .map(|partition| partition.name.clone())
            .or_else(|| {
                self.artifact_plan
                    .partitions
                    .iter()
                    .find(|partition| partition.name.ends_with("::functions"))
                    .map(|partition| partition.name.clone())
            })
            .or_else(|| self.artifact_plan.partitions.first().map(|partition| partition.name.clone()))
    }

    fn partition_count(&self) -> usize {
        self.artifact_plan.partitions.len()
    }

    fn partition(&self, partition_index: usize) -> Option<&nyar::ArtifactPartition> {
        self.artifact_plan.partitions.get(partition_index)
    }

    fn backend_requirement(&self, partition_index: usize) -> Option<nyar::PartitionBackendRequirement> {
        self.artifact_plan.backend_requirement(partition_index)
    }
}
