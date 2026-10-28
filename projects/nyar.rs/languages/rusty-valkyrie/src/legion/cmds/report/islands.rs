//! Thin wrapper: legion report charts delegate to `crate::asgard::report_build`.
use crate::{
    asgard::report_build::emit_report_chart_islands,
    legion::cmds::error::{LegionError, LegionResult},
};
use std::path::{Path, PathBuf};

use super::codegen::ChartIslandSpec;

pub use crate::asgard::report_build::{asgard_boot_head_tags, asgard_start_snippet};

/// Emit chart islands under `output_dir` and return `(route, mount_html)` pairs.
pub fn emit_chart_islands(
    output_dir: &Path,
    module_stem: &str,
    charts: &[ChartIslandSpec],
    report_project: &Path,
    roots: &[PathBuf],
) -> LegionResult<Vec<(String, String)>> {
    Ok(emit_report_chart_islands(report_project, output_dir, module_stem, charts, roots)?)
}
