//! Build plan API 载荷。

use crate::legion::{
    api::diagnostic::{LegionDiagnostic, LegionSeverity, LegionStage},
    cache::compute_artifact_hash,
    cmds::project_input::resolve_project_path,
    workspace::planner::{BuildPlan, BuildRequest, PlannerError, PlannerResult, WorkspaceResolver},
};
use serde::Serialize;

use super::request::PlanRequest;

/// 语义源码组摘要。
#[derive(Debug, Clone, Serialize)]
pub struct LegionSemanticGroupSummary {
    /// 依赖键（`std` / `core` 或包名）。
    pub dependency_key: String,
    /// 组内源码文件数。
    pub file_count: usize,
}

/// 选定的 host provider 摘要。
#[derive(Debug, Clone, Serialize)]
pub struct LegionHostProviderSummary {
    /// 提供者符号名。
    pub symbol: String,
    /// 声明身份（调试格式）。
    pub key: String,
    /// 契约声明身份（若有）。
    pub contract_key: Option<String>,
    /// 由外部绑定推导的 capability 需求。
    pub required_capabilities: Vec<String>,
    /// 声明上的外部绑定 attribute 原文。
    pub foreign_bindings: Vec<String>,
}

/// 完整 build plan（API 合同）；内含 planner 的 `BuildPlan` 供 `compile()` 消费。
#[derive(Debug, Clone, Serialize)]
pub struct LegionBuildPlan {
    /// 工程名。
    pub project: String,
    /// 目标平台字符串。
    pub target: String,
    /// Workspace 根目录。
    pub workspace_root: String,
    /// 产物输出目录。
    pub output_dir: String,
    /// 源码文件总数。
    pub source_count: usize,
    /// 语义源码组数量。
    pub semantic_group_count: usize,
    /// 直接依赖数量。
    pub dependency_count: usize,
    /// 宿主契约声明数。
    pub host_contract_count: usize,
    /// 已选 host provider 数。
    pub selected_host_provider_count: usize,
    /// 各语义组摘要。
    pub groups: Vec<LegionSemanticGroupSummary>,
    /// 已选 host provider 列表。
    pub selected_host_providers: Vec<LegionHostProviderSummary>,
    /// 已选 provider 推导的 capability 需求（去重排序）。
    pub host_required_capabilities: Vec<String>,
    /// 与缓存键一致的源码+清单摘要。
    pub source_bundle_hash: String,
    /// 模板展开架构键（含 wasi 伪键）。
    pub compile_arch_key: String,
    /// 规划阶段诊断（含预检错误）。
    pub diagnostics: Vec<LegionDiagnostic>,
    /// Planner 原始计划（供 `compile()` 使用）。
    #[serde(skip)]
    pub inner: BuildPlan,
}

/// 解析工程并生成 plan；不进入 Compiler。
pub fn plan(request: PlanRequest) -> PlannerResult<LegionBuildPlan> {
    let project_input = resolve_project_path(&request.project_dir)?;
    let workspace = WorkspaceResolver::discover_for_project(&project_input)?;
    let build_request = BuildRequest { project_dir: project_input, target: request.target, output_dir: request.output_dir };
    let inner = workspace.build_plan(&build_request)?;
    Ok(LegionBuildPlan::from_build_plan(inner))
}

impl LegionBuildPlan {
    /// 从 workspace planner 产物构造 API plan。
    pub fn from_build_plan(inner: BuildPlan) -> Self {
        let canonical_triple = inner.project.build_target.target.as_canonical_str();
        let manifest_files = inner
            .project
            .semantic_source_groups
            .iter()
            .map(|group| group.manifest_dir.join("legion.von"))
            .chain(std::iter::once(inner.project.manifest_path.clone()))
            .collect::<Vec<_>>();
        let source_bundle_hash = compute_artifact_hash(
            &inner.project.source_files,
            &manifest_files,
            &canonical_triple,
            inner.project.build_target.wat,
            inner.project.build_target.runtime_async,
        )
        .unwrap_or_else(|error| format!("hash-error:{error}"));

        let compile_arch_key = crate::legion::cmds::build::compile_arch(&inner).to_string();

        let mut diagnostics = Vec::new();
        if inner.project.source_files.is_empty() {
            diagnostics.push(LegionDiagnostic {
                stage: LegionStage::Plan,
                code: "legion.plan.no_source_files".into(),
                severity: LegionSeverity::Error,
                message: "构建计划下没有任何源码文件".into(),
                source: Some(inner.project.manifest_path.display().to_string()),
                span: None,
                owner: "legion::workspace::planner".into(),
            });
        }

        let host_required_capabilities = {
            let mut capabilities = inner
                .project
                .selected_host_providers
                .iter()
                .flat_map(|provider| provider.required_capabilities.iter().cloned())
                .collect::<Vec<_>>();
            capabilities.sort();
            capabilities.dedup();
            capabilities
        };

        Self {
            project: inner.project.name.clone(),
            target: inner.project.build_target.target.to_string(),
            workspace_root: inner.workspace_root.display().to_string(),
            output_dir: inner.output_dir.display().to_string(),
            source_count: inner.project.source_files.len(),
            semantic_group_count: inner.project.semantic_source_groups.len(),
            dependency_count: inner.project.dependencies.len(),
            host_contract_count: inner.project.host_contracts.len(),
            selected_host_provider_count: inner.project.selected_host_providers.len(),
            groups: inner
                .project
                .semantic_source_groups
                .iter()
                .map(|group| LegionSemanticGroupSummary { dependency_key: group.dependency_key.clone(), file_count: group.source_files.len() })
                .collect(),
            selected_host_providers: inner
                .project
                .selected_host_providers
                .iter()
                .map(|provider| LegionHostProviderSummary {
                    symbol: provider.symbol.clone(),
                    key: format!("{:?}", provider.key),
                    contract_key: provider.contract_key.as_ref().map(|key| format!("{:?}", key)),
                    required_capabilities: provider.required_capabilities.clone(),
                    foreign_bindings: provider.foreign_bindings.clone(),
                })
                .collect(),
            host_required_capabilities,
            source_bundle_hash,
            compile_arch_key,
            diagnostics,
            inner,
        }
    }

    /// Planner 级硬错误（如无源码）时返回 `false`。
    pub fn is_compilable(&self) -> bool {
        self.source_count > 0 && !self.diagnostics.iter().any(|d| d.severity == LegionSeverity::Error)
    }
}
