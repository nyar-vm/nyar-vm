//! 报告静态资源：从 `valkyrie.v/projects/legion._/projects/legion.report` 加载（SSG 源）。
use crate::{
    asgard::{
        codegen::StaticRenderResult,
        ssg::project::{read_project_source, render_project_awsl_static},
    },
    legion::cmds::error::LegionResult,
};
use serde_json::Value;
use std::path::Path;

/// 报告基础样式。
pub fn report_base_css(project_dir: &Path) -> LegionResult<String> {
    Ok(read_project_source(project_dir, "assets/base.css")?)
}

/// 静态渲染报告页面 AWSL（`pages/*.awsl`）。
pub fn render_report_page(project_dir: &Path, relative: &str, component_name: &str, data: &Value) -> LegionResult<StaticRenderResult> {
    Ok(render_project_awsl_static(project_dir, relative, component_name, data)?)
}
