//! 从 Valyrie VOA 工程目录加载 AWSL/CSS 并静态渲染。
use crate::asgard::error::{AsgardError, AsgardResult};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::asgard::{codegen::StaticRenderResult, ssg::render_awsl_static};

/// `valkyrie.v` 源码库根目录的显式配置环境变量。
pub const VALKYRIE_V_ROOT_ENV: &str = "VALKYRIE_V_ROOT";

/// 读取显式配置的 `valkyrie.v` 源码库根目录。
///
/// Asgard 不携带、也不猜测语言源码库位置：模板与工程源码由调用方通过
/// `VALKYRIE_V_ROOT` 显式提供。未配置或目录不存在时返回结构化失败。
pub fn valkyrie_v_roots() -> AsgardResult<Vec<PathBuf>> {
    let Some(raw) = std::env::var_os(VALKYRIE_V_ROOT_ENV)
    else {
        return Err(AsgardError::msg(report!("未配置 `{VALKYRIE_V_ROOT_ENV}`：请显式指定 valkyrie.v 源码库根目录")));
    };
    if raw.is_empty() {
        return Err(AsgardError::msg(report!("`{VALKYRIE_V_ROOT_ENV}` 为空：请显式指定 valkyrie.v 源码库根目录")));
    }
    let root = PathBuf::from(raw);
    if !root.is_dir() {
        return Err(AsgardError::msg(report!("`{VALKYRIE_V_ROOT_ENV}` 指向的目录不存在: {}", root.display())));
    }
    Ok(vec![root])
}

/// 读取工程 `source/{relative}` 文本。
pub fn read_project_source(project_dir: &Path, relative: &str) -> AsgardResult<String> {
    let path = project_dir.join("source").join(relative);
    fs::read_to_string(&path).map_err(|e| AsgardError::msg(format!("read {}: {e}", path.display())))
}

/// 从工程静态渲染 AWSL 页面片段。
pub fn render_project_awsl_static(
    project_dir: &Path,
    source_relative: &str,
    component_name: &str,
    data: &Value,
) -> AsgardResult<StaticRenderResult> {
    let source = read_project_source(project_dir, source_relative)?;
    Ok(render_awsl_static(&source, component_name, source_relative, data))
}
