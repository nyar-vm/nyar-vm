//! `legion plan`：只做 workspace 解析与 build plan，**不**进入 Compiler。
use crate::legion::{
    CanonicalTarget,
    cmds::error::{LegionError, LegionResult},
};
use std::{path::Path, process::ExitCode};

use crate::legion::{
    api::{PlanRequest, plan},
    cmds::project_input::resolve_project_path,
};

/// `legion plan` 参数。
#[derive(Debug, Clone)]
pub struct PlanArgs {
    /// 项目目录。
    pub project_dir: std::path::PathBuf,
    /// 目标平台。
    pub target: CanonicalTarget,
}

/// 执行 `legion plan`（安全：不调用 `compile_plan`）。
pub fn run(args: &PlanArgs) -> LegionResult<ExitCode> {
    let project_input = resolve_project_path(&args.project_dir)?;
    let build_plan = plan(PlanRequest { project_dir: project_input, target: args.target.clone(), output_dir: None })?;
    if !build_plan.is_compilable() {
        return Err(LegionError::Planner(crate::legion::workspace::planner::PlannerError::NoSourceFiles));
    }
    println!("workspace: {}", crate::legion::cmds::path_for_cli_log(Path::new(&build_plan.workspace_root)));
    println!("project: {}", build_plan.project);
    println!("target: {}", build_plan.target);
    println!("sources: {}", build_plan.source_count);
    println!("semantic groups: {}", build_plan.semantic_group_count);
    println!("dependencies: {}", build_plan.dependency_count);

    for group in &build_plan.groups {
        println!("  group {}: {} file(s)", group.dependency_key, group.file_count);
    }

    println!("plan: ok (未进入 Compiler；完整 build 请用 `legion build`)");
    Ok(ExitCode::SUCCESS)
}
