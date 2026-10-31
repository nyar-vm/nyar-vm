//! `asgard.config.v` 的独立发现：显式路径优先，否则从调用目录向上找最近的配置文件。
//!
//! 本模块只做路径定位：不搜索子目录、不合并多份配置、不读取 `legion.von`，
//! 也不把 Git / Legion 工程文件 / workspace 成员表当作搜索边界。
//! 配置文件所在目录即项目根，配置内的相对路径一律相对该根解析。

use crate::asgard::error::{AsgardError, AsgardResult};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Asgard 配置文件名。
pub const ASGARD_CONFIG_FILENAME: &str = "asgard.config.v";

/// 配置发现请求。
#[derive(Debug, Clone)]
pub struct ConfigDiscoveryRequest {
    /// 向上搜索的起点目录，通常是命令调用目录。
    pub start_dir: PathBuf,
    /// 显式配置路径；有值时优先，且不做向上搜索。
    pub explicit_config: Option<PathBuf>,
}

/// 配置发现结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLocation {
    /// 选中的 `asgard.config.v` 路径。
    pub config_path: PathBuf,
    /// 项目根；等于配置文件所在目录。
    pub project_root: PathBuf,
}

/// 定位 `asgard.config.v`。
///
/// `explicit_config` 有值时只校验该路径（必须存在且是普通文件），不做向上搜索；
/// 否则从 `start_dir`（相对路径先按当前工作目录绝对化）逐级向上，每级检查
/// `<dir>/asgard.config.v`，第一个命中即返回，到达文件系统根后停止。
/// 找不到时返回错误，并在消息中列出已检查的每一级路径。
pub fn discover_asgard_config(request: ConfigDiscoveryRequest) -> AsgardResult<ConfigLocation> {
    if let Some(explicit) = request.explicit_config {
        let metadata = fs::metadata(&explicit)
            .map_err(|error| AsgardError::msg(format!("asgard: 显式配置路径 `{}` 不可用：{error}", explicit.display())))?;
        if !metadata.is_file() {
            return Err(AsgardError::msg(format!("asgard: 显式配置路径 `{}` 不是普通文件", explicit.display())));
        }
        let project_root = explicit
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| AsgardError::msg(format!("asgard: 显式配置路径 `{}` 没有父目录，无法确定项目根", explicit.display())))?;
        return Ok(ConfigLocation { config_path: explicit, project_root });
    }

    let start_dir = absolute_path(&request.start_dir)?;
    let mut checked = Vec::new();
    for dir in ancestor_dirs(&start_dir) {
        // 用文件系统元数据判定，而不是字符串比较，避免大小写/符号链接差异。
        let candidate = dir.join(ASGARD_CONFIG_FILENAME);
        checked.push(candidate.clone());
        match fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_file() => return Ok(ConfigLocation { config_path: candidate, project_root: dir }),
            _ => continue,
        }
    }

    Err(AsgardError::msg(checked_paths_message(&start_dir, &checked)))
}

/// 把相对路径按当前工作目录绝对化；已是绝对路径则原样返回。
fn absolute_path(path: &Path) -> AsgardResult<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::path::absolute(path).map_err(|error| AsgardError::msg(format!("asgard: 无法绝对化路径 `{}`：{error}", path.display())))
}

/// 沿父目录链从 `start_dir` 走到文件系统根（含根目录）。
fn ancestor_dirs(start_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut current = Some(start_dir.to_path_buf());
    while let Some(dir) = current {
        current = dir.parent().map(Path::to_path_buf);
        dirs.push(dir);
    }
    dirs
}

/// 构造“未找到配置”的错误消息，逐级列出已检查的绝对路径。
fn checked_paths_message(start_dir: &Path, checked: &[PathBuf]) -> String {
    let mut message = format!("asgard: 从 `{}` 向上未找到 `{ASGARD_CONFIG_FILENAME}`。已检查：", start_dir.display());
    for path in checked {
        message.push_str("\n  - ");
        message.push_str(&path.display().to_string());
    }
    message.push_str("\n提示：请显式传入配置路径，或在项目根创建 `asgard.config.v`（Asgard 不会向下搜索子目录）。");
    message
}
