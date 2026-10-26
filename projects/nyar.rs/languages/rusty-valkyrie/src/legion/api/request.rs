//! API 请求类型（无 argv）。

use nyar::CanonicalTarget;
use std::path::PathBuf;

/// `plan()` 输入。
#[derive(Debug, Clone)]
pub struct PlanRequest {
    /// 项目目录、包根或单脚本路径。
    pub project_dir: PathBuf,
    /// 目标平台。
    pub target: CanonicalTarget,
    /// 可选产物目录（默认由 planner 决定）。
    pub output_dir: Option<PathBuf>,
}

/// `compile()` 选项。
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// 是否打印编译进度类日志（CLI 用）。
    pub verbose: bool,
    /// 是否写出 host 选择调试副产物。
    pub debug_artifacts: bool,
}
