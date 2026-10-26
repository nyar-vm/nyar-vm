//! 源码分流：AWSL 与 V 逻辑分离。
use crate::asgard::error::{AsgardError, AsgardResult};
use std::{fs, path::Path};

/// 加载单个 AWSL 文件内容。
pub fn read_awsl_file(path: &Path) -> AsgardResult<String> {
    let content = fs::read_to_string(path).map_err(|e| AsgardError::msg(format!("读取 AWSL 失败: {}: {e}", path.display())))?;
    Ok(content.strip_prefix('\u{FEFF}').unwrap_or(&content).to_string())
}
