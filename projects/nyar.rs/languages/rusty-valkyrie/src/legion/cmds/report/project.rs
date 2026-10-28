//! Legion 报告工程路径（`valkyrie.v/projects/legion._/projects/legion.report`）。
use crate::{
    asgard::ssg::project::VALKYRIE_V_ROOT_ENV,
    legion::{
        cmds::error::{LegionError, LegionResult},
        workspace::planner::WorkspaceResolver,
    },
};
use std::path::{Path, PathBuf};

/// 报告模板工程在源码库根下的相对路径。
const REPORT_PROJECT_RELATIVE: &str = "projects/legion._/projects/legion.report";

/// 定位报告模板所用的源码库根目录。
///
/// 优先消费已发现的 workspace 事实：`legions.von` 成员路径向上最近的
/// `legions.von` 即成员所属源码库根（例如 `leetcode.v` 的成员落在
/// `../valkyrie.v`）。只有 workspace 推导不出可用的根时，才回退到
/// `VALKYRIE_V_ROOT`；失败信息列出全部已尝试的发现路径。
pub fn report_roots(workspace: Option<&WorkspaceResolver>, project_dir: &Path) -> LegionResult<Vec<PathBuf>> {
    let mut roots = workspace.map(WorkspaceResolver::source_library_roots).unwrap_or_default();
    let mut attempted: Vec<String> = roots.iter().map(|root| root.display().to_string()).collect();
    if roots.is_empty() {
        attempted.push(format!("从 `{}` 向上查找 `legions.von` 未发现 workspace 成员源码库", project_dir.display()));
    }

    let env_root = std::env::var_os(VALKYRIE_V_ROOT_ENV).filter(|raw| !raw.is_empty()).map(PathBuf::from);
    let workspace_has_report = roots.iter().any(|root| report_project_in(root).is_some());
    if !workspace_has_report {
        match &env_root {
            Some(root) if root.is_dir() => {
                attempted.push(format!("{VALKYRIE_V_ROOT_ENV}={}", root.display()));
                if !roots.iter().any(|candidate| candidate == root) {
                    roots.push(root.clone());
                }
            }
            Some(root) => attempted.push(format!("{VALKYRIE_V_ROOT_ENV}={}（不是目录）", root.display())),
            None => attempted.push(format!("{VALKYRIE_V_ROOT_ENV} 未设置")),
        }
    }

    if roots.is_empty() {
        return Err(LegionError::msg(report!("未找到报告模板源码库根目录；已尝试: {}", attempted.join("、"))));
    }
    Ok(roots)
}

/// `valkyrie.v/projects/legion._/projects/legion.report` 工程目录。
pub fn legion_report_project_dir(roots: &[PathBuf]) -> LegionResult<PathBuf> {
    for root in roots {
        if let Some(project) = report_project_in(root) {
            return Ok(project);
        }
    }
    let attempted = roots.iter().map(|root| root.display().to_string()).collect::<Vec<_>>().join("、");
    Err(LegionError::msg(report!("未找到 `{REPORT_PROJECT_RELATIVE}`；已尝试的源码库根目录: {attempted}")))
}

fn report_project_in(root: &Path) -> Option<PathBuf> {
    let nested = root.join(REPORT_PROJECT_RELATIVE);
    (nested.is_dir() && nested.join("legion.von").is_file()).then_some(nested)
}
