//! 测试用临时目录：进程内唯一命名，drop 时递归清理。
//!
//! 本 crate 不引入外部临时目录依赖；只依赖标准库与本进程计数器。

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 一个在 drop 时删除自身目录的临时目录。
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// 目录路径。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// 创建唯一临时目录。
pub fn tempdir() -> std::io::Result<TempDir> {
    let index = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("compiler-emitter-{}-{index}", std::process::id()));
    std::fs::create_dir_all(&path)?;
    Ok(TempDir { path })
}
