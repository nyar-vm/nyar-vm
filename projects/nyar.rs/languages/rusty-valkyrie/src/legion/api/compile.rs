//! `compile()` API 载荷。

use crate::{
    compiler::emitter::pipeline::CompilerCompileEvidence,
    legion::{
        api::{
            diagnostic::{LegionDiagnostic, LegionSeverity, LegionStage},
            plan::LegionBuildPlan,
            request::CompileOptions,
        },
        cmds::{
            build::compile_plan,
            error::{LegionError, LegionResult},
        },
        parse_von,
        workspace::planner::PlannerError,
    },
};
use nyar::CanonicalTarget;
use serde::Serialize;
use std::{fs, path::PathBuf};

/// 单阶段执行结果。
#[derive(Debug, Clone, Serialize)]
pub struct LegionStageStatus {
    /// 阶段。
    pub stage: LegionStage,
    /// 是否成功。
    pub success: bool,
    /// 可选说明。
    pub detail: Option<String>,
}

/// 编译报告（API 合同）。
#[derive(Debug, Serialize)]
pub struct LegionCompileReport {
    /// 工程名。
    pub project: String,
    /// 目标平台。
    pub target: String,
    /// 输出目录。
    pub output_dir: String,
    /// 整体是否成功。
    pub success: bool,
    /// 各阶段状态。
    pub stages: Vec<LegionStageStatus>,
    /// 结构化诊断。
    pub diagnostics: Vec<LegionDiagnostic>,
    /// 落盘产物路径。
    pub artifacts: Vec<String>,
    /// 入口符号。
    pub entry_symbol: Option<String>,
    /// 编译证据 sidecar（若已写出）。
    pub evidence: Option<CompilerCompileEvidence>,
    /// 本次构建物化所需的宿主 capability id（来自 compile evidence 中的 provider 绑定）。
    pub host_required_capabilities: Vec<String>,
    /// 驱动层原始报告（供 CLI / unity export）；不参与序列化。
    #[serde(skip)]
    pub driver: Option<nyar_emitter::DriverCompileReport>,
}

fn host_required_capabilities_from_evidence(evidence: &CompilerCompileEvidence) -> Vec<String> {
    let mut capabilities: Vec<String> = evidence
        .host_provider_bindings
        .iter()
        .flat_map(|binding| binding.required_capabilities.iter().cloned())
        .collect();
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

/// 在已有 plan 上执行编译。
pub fn compile(plan: &LegionBuildPlan, options: CompileOptions) -> LegionResult<LegionCompileReport> {
    if plan.source_count == 0 {
        return Err(LegionError::Planner(PlannerError::NoSourceFiles));
    }

    let mut stages = vec![LegionStageStatus {
        stage: LegionStage::SourceSnapshot,
        success: true,
        detail: Some(format!("semantic_groups={}", plan.semantic_group_count)),
    }];

    let driver = match compile_plan(&plan.inner, options.verbose) {
        Ok(report) => {
            stages.push(LegionStageStatus { stage: LegionStage::Compile, success: true, detail: Some("emitter driver completed".into()) });
            stages.push(LegionStageStatus { stage: LegionStage::Cache, success: true, detail: None });
            report
        }
        Err(error) => {
            stages.push(LegionStageStatus { stage: LegionStage::Compile, success: false, detail: Some(error.to_string()) });
            return Ok(LegionCompileReport {
                project: plan.project.clone(),
                target: plan.target.clone(),
                output_dir: plan.output_dir.clone(),
                success: false,
                stages,
                diagnostics: vec![diagnostic_from_legion_error(&error)],
                artifacts: Vec::new(),
                entry_symbol: None,
                evidence: None,
                host_required_capabilities: Vec::new(),
                driver: None,
            });
        }
    };

    let artifacts: Vec<String> =
        driver.artifacts.artifacts.iter().map(|artifact| plan.inner.output_dir.join(&artifact.path).display().to_string()).collect();

    stages.push(LegionStageStatus {
        stage: LegionStage::Artifact,
        success: !artifacts.is_empty(),
        detail: Some(format!("count={}", artifacts.len())),
    });

    let evidence = read_compile_evidence(&plan.inner.output_dir);
    let host_required_capabilities = evidence.as_ref().map(host_required_capabilities_from_evidence).unwrap_or_default();

    Ok(LegionCompileReport {
        project: plan.project.clone(),
        target: plan.target.clone(),
        output_dir: plan.output_dir.clone(),
        success: true,
        stages,
        diagnostics: plan.diagnostics.clone(),
        artifacts,
        entry_symbol: driver.entry_symbol.clone(),
        evidence,
        host_required_capabilities,
        driver: Some(driver),
    })
}

/// 规划并编译（单入口）。
pub fn compile_project(
    project_dir: impl AsRef<std::path::Path>,
    target: CanonicalTarget,
    output_dir: Option<PathBuf>,
    options: CompileOptions,
) -> LegionResult<LegionCompileReport> {
    let plan = super::plan::plan(super::request::PlanRequest { project_dir: project_dir.as_ref().to_path_buf(), target, output_dir })
        .map_err(LegionError::Planner)?;
    compile(&plan, options)
}

fn read_compile_evidence(output_dir: &std::path::Path) -> Option<CompilerCompileEvidence> {
    let path = output_dir.join("compile-evidence.txt");
    let text = fs::read_to_string(path).ok()?;
    parse_von(&text).ok()
}

fn diagnostic_from_legion_error(error: &LegionError) -> LegionDiagnostic {
    LegionDiagnostic::compile_error("legion.compile.failed", error.to_string())
}
