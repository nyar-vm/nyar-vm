//! 报告文件写出。
use crate::legion::cmds::error::{LegionError, LegionResult};
use std::{fs, path::Path};

/// 原子写出文本（临时文件 + rename）。
pub fn atomic_write_all_text(file_path: &Path, content: &str) -> LegionResult<()> {
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).map_err(|e| LegionError::msg(format!("创建目录失败 {}: {e}", parent.display())))?;
    }

    let parent = file_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("index.html");
    let temp_path = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));

    fs::write(&temp_path, content).map_err(|e| LegionError::msg(format!("写入临时文件失败 {}: {e}", temp_path.display())))?;
    fs::rename(&temp_path, file_path)
        .map_err(|e| LegionError::msg(format!("原子重命名失败 {} -> {}: {e}", temp_path.display(), file_path.display())))?;
    Ok(())
}
