//! `legion plan`：只做 workspace 解析与 build plan，**不**进入 Compiler。
use crate::legion::{
    CanonicalTarget,
    cmds::error::{LegionError, LegionResult},
};
use std::{path::Path, process::ExitCode};

use crate::legion::{api::plan_project, cmds::project_input::resolve_project_path};

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
    let summary = plan_project(&project_input, args.target.clone())?;

    println!("workspace: {}", crate::legion::cmds::path_for_cli_log(Path::new(&summary.workspace_root)));
    println!("project: {}", summary.project);
    println!("target: {}", summary.target);
    println!("sources: {}", summary.source_count);
    println!("semantic groups: {}", summary.semantic_group_count);
    println!("dependencies: {}", summary.dependency_count);

    for group in &summary.groups {
        println!("  group {}: {} file(s)", group.dependency_key, group.file_count);
    }

    println!("plan: ok (未进入 Compiler；完整 build 请用 `legion build`)");
    Ok(ExitCode::SUCCESS)
}
