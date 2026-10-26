use crate::legion::cmds::error::{LegionError, LegionResult};
use std::{fs, path::PathBuf, process::ExitCode};

use crate::legion::workspace::planner::WorkspaceResolver;

/// `legion clean` arguments.
#[derive(Debug, Clone)]
pub struct CleanArgs {
    /// Project directory.
    pub project_dir: PathBuf,
    /// Clean all workspace members.
    pub workspace: bool,
}

/// Run `legion clean`.
pub fn run(args: &CleanArgs) -> LegionResult<ExitCode> {
    if args.workspace {
        let workspace = WorkspaceResolver::discover(&args.project_dir)?;
        let mut cleaned = 0usize;
        for member in workspace.member_manifest_dirs() {
            cleaned += clean_project_dirs(&member)?;
        }
        println!("已清理 workspace 成员 {} 个目录", cleaned);
        return Ok(ExitCode::SUCCESS);
    }

    let cleaned = clean_project_dirs(&args.project_dir)?;
    println!("已清理 {} 个目录", cleaned);
    Ok(ExitCode::SUCCESS)
}

fn clean_project_dirs(project_dir: &PathBuf) -> LegionResult<usize> {
    let mut cleaned = 0usize;
    for relative in ["dist", ".cache"] {
        let path = project_dir.join(relative);
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
            println!("clean: {}", path.display());
            cleaned += 1;
        }
    }
    Ok(cleaned)
}
