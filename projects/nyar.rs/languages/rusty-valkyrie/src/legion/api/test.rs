//! 测试 API（LegionTestReport 合同 + `test_project` 入口）。
//!
//! 当前阶段作为 API-first 骨架：底层复用 `cmds::test_engine` 的公开
//! `run_tests_for_project`，把 CLI 现在直接消费的 tuple 结果提升为
//! 结构化 `LegionTestReport`，便于 N-API / IDE / 未来远程 runner 复用。
//!
//! 下一阶段再把 compile_external_test_session / run_external_test_in_session
//! 拆成 API 可控的子步骤（API 层自己持有 session）。

use crate::legion::{
    api::plan::LegionBuildPlan,
    cmds::{
        error::{LegionError, LegionResult},
        project_input::resolve_project_path,
        report::TestResultEntry,
        test_engine::{resolve_test_targets, run_tests_for_project},
    },
    workspace::planner::WorkspaceResolver,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

/// `test_project` 输入（无 argv 解析）。
#[derive(Debug, Clone, Default)]
pub struct LegionTestSelector {
    /// 目标标签列表；`None` 时 API 层用默认（= `legion`）。
    ///
    /// CLI 在传入前会先做 `--target all` / `clr,jvm` 展开。
    pub targets: Option<Vec<String>>,
    /// 测试名过滤器（大小写不敏感子串匹配）。
    pub filter: Option<String>,
    /// 显式 runner 覆盖列表，格式 `target=command`。
    pub runner_overrides: Vec<String>,
    /// 详细输出（CLI 用）。
    pub verbose: bool,
}

impl LegionTestSelector {
    /// 得到实际要跑的 target 列表。
    pub fn resolved_targets(&self) -> Vec<String> {
        match &self.targets {
            Some(list) if !list.is_empty() => list.clone(),
            _ => resolve_test_targets(None),
        }
    }
}

/// 单条测试用例结果（API 合同）。
#[derive(Debug, Clone, Serialize)]
pub struct LegionTestCaseResult {
    /// 测试函数名；workspace 模式下带 `<member>::` 前缀。
    pub name: String,
    /// 最终状态：`pass` / `fail` / `skip` / `compile_error`。
    pub status: String,
    /// 失败/跳过原因（若有）。
    pub error: Option<String>,
    /// 运行目标标签。
    pub target: String,
}

/// 单个 target 上的测试结果集。
#[derive(Debug, Clone, Serialize)]
pub struct LegionTargetTestResults {
    /// 目标标签（`legion` / `clr` / `jvm` / `node` …）。
    pub target: String,
    /// 该 target 上的结果。
    pub cases: Vec<LegionTestCaseResult>,
    /// 通过数。
    pub passed: usize,
    /// 失败数（含 compile_error）。
    pub failed: usize,
    /// 跳过数。
    pub skipped: usize,
}

/// 汇总测试报告（API 合同）。
#[derive(Debug, Serialize)]
pub struct LegionTestReport {
    /// 工程名。
    pub project: String,
    /// 各 target 结果。
    pub targets: Vec<LegionTargetTestResults>,
    /// 汇总通过数。
    pub total_passed: usize,
    /// 汇总失败数。
    pub total_failed: usize,
    /// 汇总跳过数。
    pub total_skipped: usize,
    /// 底层退出码（供 CLI `main` 直接返回）。
    #[serde(skip)]
    pub exit_code: ExitCode,
}

impl LegionTestReport {
    /// 把所有 target / 所有用例展平成 `TestResultEntry` 列表。
    ///
    /// CLI HTML 报告需要 `TestResultEntry`，API 层自己不直接依赖它做
    /// 结构，只在 CLI 渲染层做一次薄转换。
    pub fn flatten_entries(&self) -> Vec<TestResultEntry> {
        let mut out = Vec::new();
        for t in &self.targets {
            for case in &t.cases {
                out.push(TestResultEntry {
                    name: case.name.clone(),
                    status: case.status.clone(),
                    error: case.error.clone(),
                    target: case.target.clone(),
                });
            }
        }
        out
    }
}

/// 在已规划好的工程上运行测试。
///
/// `plan` 仅用于反查工程目录与工程名；测试构建计划由
/// `WorkspaceResolver::build_test_plan` 在 `run_tests_for_project` 内部
/// 重新生成（会把 `test/` 目录下的 `.v` 文件纳入编译输入）。
pub fn test_project(plan: &LegionBuildPlan, selector: LegionTestSelector) -> LegionResult<LegionTestReport> {
    // 从 plan 的 inner BuildPlan 反查工程目录。
    let project_dir = plan
        .inner
        .project
        .manifest_path
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| LegionError::msg(report!("plan 缺少 manifest_path 父目录")))?;

    test_project_at(&project_dir, &plan.project, &selector)
}

/// 在任意工程路径上跑测试；不依赖预先存在的 `LegionBuildPlan`。
///
/// CLI `legion test` 与 N-API `test_project(project_dir, selector)` 都走到这里。
pub fn test_project_at(
    project_dir: &Path,
    project_name: &str,
    selector: &LegionTestSelector,
) -> LegionResult<LegionTestReport> {
    let resolved = resolve_project_path(project_dir)?;
    let workspace = WorkspaceResolver::discover_for_project(&resolved)?;
    let targets = selector.resolved_targets();

    let (passed, failed, skipped, entries) = run_tests_for_project(
        &workspace,
        &resolved,
        selector.filter.as_deref(),
        &targets,
        &selector.runner_overrides,
        selector.verbose,
    );

    let grouped = group_entries_by_target(&entries);
    let exit_code = if failed > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS };

    Ok(LegionTestReport {
        project: project_name.to_string(),
        targets: grouped,
        total_passed: passed,
        total_failed: failed,
        total_skipped: skipped,
        exit_code,
    })
}

fn group_entries_by_target(entries: &[TestResultEntry]) -> Vec<LegionTargetTestResults> {
    let mut seen_targets: Vec<String> = Vec::new();
    for entry in entries {
        if !seen_targets.iter().any(|t| t == &entry.target) {
            seen_targets.push(entry.target.clone());
        }
    }
    let mut out = Vec::new();
    for target in seen_targets {
        let cases: Vec<LegionTestCaseResult> = entries
            .iter()
            .filter(|e| e.target == target)
            .map(test_result_entry_to_api)
            .collect();
        let mut t_passed = 0usize;
        let mut t_failed = 0usize;
        let mut t_skipped = 0usize;
        for case in &cases {
            match case.status.as_str() {
                "pass" => t_passed += 1,
                "skip" => t_skipped += 1,
                _ => t_failed += 1, // fail / compile_error
            }
        }
        out.push(LegionTargetTestResults { target, cases, passed: t_passed, failed: t_failed, skipped: t_skipped });
    }
    out
}

fn test_result_entry_to_api(entry: &TestResultEntry) -> LegionTestCaseResult {
    LegionTestCaseResult {
        name: entry.name.clone(),
        status: entry.status.clone(),
        error: entry.error.clone(),
        target: entry.target.clone(),
    }
}
