//! Legion 报告工程路径（`valkyrie.v/projects/legion._/projects/legion.report`）。
use crate::{
    asgard::ssg::project::valkyrie_v_roots,
    legion::cmds::error::{LegionError, LegionResult},
};
use std::path::PathBuf;

/// `valkyrie.v/projects/legion._/projects/legion.report` 工程目录。
pub fn legion_report_project_dir() -> LegionResult<PathBuf> {
    let roots = valkyrie_v_roots()?;
    for root in &roots {
        let nested = root.join("projects/legion._/projects/legion.report");
        if nested.is_dir() && nested.join("legion.von").is_file() {
            return Ok(nested);
        }
    }
    Err(LegionError::msg(report!("missing Valyrie project: projects/legion._/projects/legion.report")))
}
