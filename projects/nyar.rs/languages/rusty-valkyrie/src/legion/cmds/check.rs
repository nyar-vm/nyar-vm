use crate::legion::cmds::{
    build::{BuildArgs, run as run_build},
    error::{LegionError, LegionResult},
};
use std::process::ExitCode;

/// `legion check` arguments (compile / plan validation, currently reuses build).
#[derive(Debug, Clone)]
pub struct CheckArgs {
    /// Project directory.
    pub project_dir: std::path::PathBuf,
    /// 已冻结的解析目标（与 build 同一事实源：平台身份 + 产物/宿主合同）。
    pub resolved: crate::legion::ResolvedTarget,
    /// Verbose progress (maps to build debug artifacts currently unused).
    pub verbose: bool,
}

/// Run `legion check`.
pub fn run(args: &CheckArgs) -> LegionResult<ExitCode> {
    println!("check：验证项目可编译性（复用 build）");
    let build_args = BuildArgs {
        project_dir: args.project_dir.clone(),
        resolved: args.resolved.clone(),
        output_dir: None,
        debug_artifacts: args.verbose,
    };
    run_build(&build_args)
}
