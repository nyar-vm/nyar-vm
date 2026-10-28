//! `legion test` —— 运行项目测试并生成 HTML 报告。
//!
//! 编译与执行委托给 `api::test::test_project_at`；CLI 只负责：
//! - argv → `LegionTestSelector`
//! - workspace / 单项目两种入口分发
//! - 控制台汇总打印
//! - HTML 报告渲染（`cmds::report` 负责）
use crate::legion::{
    api::{LegionTestSelector, test::test_project_at},
    cmds::{
        error::{LegionError, LegionResult},
        project_input::resolve_project_path,
        report::{TestResultEntry, finish_standalone_report, render_test_report, report_roots},
        test_engine::resolve_test_targets,
    },
    workspace::planner::WorkspaceResolver,
};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

/// `legion test` 命令参数。
#[derive(Debug, Clone)]
pub struct TestArgs {
    /// 项目或 workspace 目录。
    pub project_dir: PathBuf,
    /// 测试名称过滤器。
    pub filter: Option<String>,
    /// 编译目标（逗号分隔，或 `all`）。
    pub target: Option<String>,
    /// 显式运行器，格式 `target=command`。
    pub runner: Vec<String>,
    /// 详细输出。
    pub verbose: bool,
    /// 额外产出单文件 HTML（内联 css/js/wasm，便于离线打开）。
    pub standalone: bool,
}

/// 执行 `legion test`。
pub fn run(args: &TestArgs) -> LegionResult<ExitCode> {
    let project_dir = resolve_project_path(&args.project_dir)?;
    let targets = resolve_test_targets(args.target.as_deref());
    let workspace = WorkspaceResolver::discover_for_project(&project_dir)?;
    // 报告模板与测试共用同一份 workspace 发现事实；`VALKYRIE_V_ROOT` 仅作兜底。
    let roots = report_roots(Some(&workspace), &project_dir)?;

    let selector = LegionTestSelector {
        targets: Some(targets.clone()),
        filter: args.filter.clone(),
        runner_overrides: args.runner.clone(),
        verbose: args.verbose,
    };

    let (passed, failed, skipped, results) = if is_workspace_root(&project_dir, &workspace) {
        run_workspace_tests_via_api(&workspace, &project_dir, &selector)?
    }
    else {
        let member_name = project_dir.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();
        println!("--- {member_name} ---");
        let report = test_project_at(&project_dir, &member_name, &selector)?;
        // report 自己已经按 target 汇总过，CLI 直接用 totals + flatten。
        (report.total_passed, report.total_failed, report.total_skipped, report.flatten_entries())
    };

    println!();
    println!("测试报告：{passed} 通过, {failed} 失败, {skipped} 跳过");

    let report_dir = report_output_dir(&args.project_dir, &project_dir);
    let project_name = project_dir.file_stem().or_else(|| project_dir.file_name()).and_then(|n| n.to_str()).unwrap_or("project");
    render_test_report(&report_dir, project_name, &results, &roots)?;
    println!("HTML 测试报告已生成：{}", report_dir.join("index.html").display());
    finish_standalone_report(&report_dir, args.standalone)?;

    Ok(if failed > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

fn run_workspace_tests_via_api(
    workspace: &WorkspaceResolver,
    workspace_dir: &Path,
    selector: &LegionTestSelector,
) -> LegionResult<(usize, usize, usize, Vec<TestResultEntry>)> {
    let members = workspace.member_manifest_dirs();
    if members.is_empty() {
        println!("错误：legions.von 中无 members");
        return Ok((0, 0, 0, Vec::new()));
    }

    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut total_skipped = 0usize;
    let mut all_results = Vec::new();

    for member_dir in members {
        let member_name = member_dir.file_name().and_then(|n| n.to_str()).unwrap_or("member").to_string();
        println!("--- {member_name} ---");
        let member_report = test_project_at(&member_dir, &member_name, selector)?;

        total_passed += member_report.total_passed;
        total_failed += member_report.total_failed;
        total_skipped += member_report.total_skipped;

        for mut entry in member_report.flatten_entries() {
            entry.name = format!("{member_name}::{}", entry.name);
            all_results.push(entry);
        }
        println!();
    }

    let _ = workspace_dir;
    Ok((total_passed, total_failed, total_skipped, all_results))
}

fn report_output_dir(arg_project_dir: &Path, resolved_project_dir: &Path) -> PathBuf {
    if arg_project_dir.extension().and_then(|e| e.to_str()) == Some("v") {
        arg_project_dir.parent().unwrap_or(arg_project_dir).join("dist").join("legion-test")
    }
    else {
        resolved_project_dir.join("dist").join("legion-test")
    }
}

fn is_workspace_root(project_dir: &Path, workspace: &WorkspaceResolver) -> bool {
    workspace.is_at_workspace_root(project_dir)
}
