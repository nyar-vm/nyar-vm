//! SSG 渲染源码的 OVM 编译：计划（含 capability 声明）→ 闭包 → 产物字节。
//!
//! 本路径是唯一声明 `asgard.ssg.dom` capability 的构建：feature 经
//! `BuildRequest.enabled_features` 进入既有 gate 求值，`asgard.render.host.ovm`
//! 的 provider 只在这里被选择；普通 `legion build` 不声明该 feature，
//! 引用合同族即因无 provider 绑定而结构化失败（planner 零结构改动）。

use std::{fs, path::Path};

use nyar::{ArtifactKind, CanonicalTarget};
use nyar_emitter::nyar_backend_wasi::WasmPackageKind;

use crate::{
    asgard::{
        compile::{GeneratedEntrySurface, GeneratedSource, compiler_input_for_with_dependencies, primary_artifact_name},
        error::{AsgardError, AsgardResult},
    },
    compiler::{
        emitter::pipeline::{CompilerBuildContext, compile_source_groups_to_artifacts},
        hir::lowering::ValkyrieCompiler,
    },
    legion::workspace::{
        compile_source_snapshot,
        planner::{BuildRequest, WorkspaceResolver, compiler_host_provider_bindings},
    },
};

/// SSG 构建宿主声明的 capability：`asgard.render` 合同族 OVM provider 的门禁 feature。
pub const SSG_DOM_FEATURE: &str = "asgard.ssg.dom";

/// OVM 目标（进程内 Odin VM 执行）。
fn ovm_target() -> AsgardResult<CanonicalTarget> {
    CanonicalTarget::parse("ovm").map_err(|error| AsgardError::msg(report!("解析 OVM 目标失败: {error}")))
}

/// 编译 SSG 渲染源码为 `.odin` 产物并返回字节。
///
/// `scratch_dir` 是构建期中间产物目录（`.odin`、证据文件），不进交付产物树。
pub(crate) fn compile_ssg_ovm_bundle(
    project_dir: &Path,
    generated: GeneratedSource<'_>,
    scratch_dir: &Path,
    module_name: &str,
) -> AsgardResult<Vec<u8>> {
    let target = ovm_target()?;
    let resolver =
        WorkspaceResolver::discover_for_project(project_dir).map_err(|error| AsgardError::msg(report!("解析项目工作区失败: {error}")))?;
    let plan = resolver
        .build_plan(&BuildRequest {
            project_dir: project_dir.to_path_buf(),
            target: target.clone(),
            output_dir: Some(scratch_dir.to_path_buf()),
            enabled_features: [SSG_DOM_FEATURE.to_string()].into_iter().collect(),
        })
        .map_err(|error| AsgardError::msg(report!("解析 SSG 构建计划失败: {error}")))?;
    let groups = compile_source_snapshot(&plan.project.semantic_source_groups)
        .map_err(|error| AsgardError::msg(report!("读取项目源码快照失败: {error}")))?;
    let host_bindings =
        compiler_host_provider_bindings(&plan.project.selected_host_providers).map_err(|error| AsgardError::msg(report!("{error}")))?;
    let gated_out_host_providers = crate::legion::workspace::planner::compiler_gated_out_host_providers(&plan.project.gated_out_host_providers);

    // 生成代码直接引用 `asgard.render` 合同族声明：合同包必须在生成组的显式
    // 依赖闭包内（生成组默认只见应用组自身声明，不见应用组的依赖）。
    let contract_keys =
        groups.iter().filter(|group| group.name == "asgard.render").map(|group| group.dependency_key.clone()).collect::<Vec<_>>();
    if contract_keys.is_empty() {
        return Err(AsgardError::msg(report!(
            "项目依赖闭包缺少 `asgard.render` 合同族声明包：SSG 渲染经 OVM 执行，工程必须在 legion.von 声明 `asgard.render` 依赖"
        )));
    }

    let target_profile = target.to_profile(None);
    let build_context =
        CompilerBuildContext::new(target.arch.as_str(), target.clone(), nyar::ClrSuspendStrategy::default(), WasmPackageKind::Library)
            .with_selected_host_providers(host_bindings)
            .with_gated_out_host_providers(gated_out_host_providers);
    let input = compiler_input_for_with_dependencies(&groups, generated, GeneratedEntrySurface::GeneratedOnly, &contract_keys)?;
    fs::create_dir_all(scratch_dir)
        .map_err(|error| AsgardError::msg(report!("创建 SSG 中间产物目录失败 {}: {error}", scratch_dir.display())))?;
    let artifact = compile_source_groups_to_artifacts(
        &ValkyrieCompiler::default().with_suppressed_imported_builtin_short_names(),
        &input,
        &build_context,
        scratch_dir,
        module_name,
        true,
        target_profile.artifact_policy.generate_runtime_config,
    )?;
    // SSG 以 library 形态编译 OVM 产物，产物合同为 `Library`而非 `Executable`。
    let artifact_name = primary_artifact_name(&artifact.driver, ArtifactKind::Library)?;
    let extension = &target_profile.artifact_policy.primary_extension;
    let filename = if artifact_name.ends_with(extension.as_str()) { artifact_name } else { format!("{artifact_name}{extension}") };
    let artifact_path = scratch_dir.join(&filename);
    fs::read(&artifact_path).map_err(|error| AsgardError::msg(report!("读取 SSG OVM 产物失败 {}: {error}", artifact_path.display())))
}
