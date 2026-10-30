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
    compiled_program: nyar_types::CompiledProgram,
    artifact_plan: ArtifactPartitionPlan,
    wasm_package_kind: nyar_emitter::nyar_backend_wasi::WasmPackageKind,
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
    let compiled_program =
        compiler.compile_source_groups_to_program_with_host_bindings(groups, &context.selected_host_providers, &context.arch)?;
    let artifact_plan = plan_artifacts_from_compiled_program(&compiled_program, context.target.clone(), context.clr_suspend_strategy)?;
    let bundle = CompilerBuildBundle {
        compiled_program,
        artifact_plan,
        wasm_package_kind: context.wasm_package_kind,
        test_entry_names: context.test_entry_names.clone(),
    };
    validate_artifact_surface(&bundle, context.wasm_package_kind)?;
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

fn validate_artifact_surface(
    bundle: &CompilerBuildBundle,
    wasm_package_kind: nyar_emitter::nyar_backend_wasi::WasmPackageKind,
) -> PipelineResult<()> {
    if bundle.artifact_plan.target.to_profile(None).backend_family != TargetBackendFamily::Wasm {
        return Ok(());
    }
    let (export_count, entry_count) = bundle.surface_counts();
    match wasm_package_kind {
        nyar_emitter::nyar_backend_wasi::WasmPackageKind::Library if export_count == 0 => {
            Err(PipelineError::Surface(ArtifactSurfaceError::LibraryMissingExport))
        }
        nyar_emitter::nyar_backend_wasi::WasmPackageKind::Binary if entry_count == 0 => {
            Err(PipelineError::Surface(ArtifactSurfaceError::BinaryMissingEntry))
        }
        _ => Ok(()),
    }
}

impl FrontendBuildBundle for CompilerBuildBundle {
    fn planned_partitions(&self) -> &dyn PlannedArtifactPartitionsView {
        self
    }

    fn wasm_package_kind(&self) -> nyar_emitter::nyar_backend_wasi::WasmPackageKind {
        self.wasm_package_kind
    }

    fn test_entry_names(&self) -> Vec<String> {
        self.test_entry_names.clone()
    }

    fn assemble_fragment_for_partition(
        &self,
        partition_index: usize,
    ) -> std::result::Result<nyar::AssembledFragment, nyar_emitter::EmitterError> {
        assemble_fragment(&self.compiled_program, &self.artifact_plan, partition_index).map_err(|message| message)
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
