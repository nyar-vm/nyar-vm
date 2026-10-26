//! 解析 CLI 传入的项目目录或单脚本 `.v` 路径。
use crate::legion::workspace::{
    planner::{PlannerError, PlannerResult},
    script::is_script_path,
};
use std::path::{Path, PathBuf};

/// 接受项目目录、`legion.von` 旁的单脚本目录，或内嵌清单的 `.v` 文件。
pub fn resolve_project_path(input: &Path) -> PlannerResult<PathBuf> {
    let canonical = input.canonicalize().unwrap_or_else(|_| input.to_path_buf());

    if is_script_path(&canonical) {
        return Ok(canonical);
    }

    if canonical.join("legion.von").is_file() || canonical.join("legions.von").is_file() || canonical.join("test").is_dir() {
        return Ok(canonical);
    }

    let sidecar = canonical.join("solution.v");
    if sidecar.is_file() {
        return Ok(sidecar);
    }

    Err(PlannerError::InvalidProjectInput { path: input.to_path_buf() })
}
