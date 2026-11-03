#![doc = include_str!("readme.md")]

pub mod error;

/// 审计依赖漏洞与许可证。
pub mod audit;
pub mod bench;
pub mod bootstrap;
/// 构建项目并落盘产物。
pub mod build;
/// 检查项目可编译性。
pub mod check;
/// 清理构建产物目录。
pub mod clean;
pub mod cov;
pub mod doc;
pub mod fmt;
/// 依赖安装/添加/移除/更新。
pub mod install;
/// 前端语义 lint（不生成后端产物）。
pub mod lint;
/// 注册表登录/登出/whoami。
pub mod login;
pub mod plan;
pub mod project_input;
/// 发布包到注册表。
pub mod publish;
/// 管理注册表源 endpoint。
pub mod registry;
pub mod report;
/// 运行项目产物。
pub mod run;
/// 搜索/查看注册表中的包。
pub mod search;
pub mod source_hygiene;
pub mod spy;
pub mod test;
pub mod test_engine;
/// 注册表认证管理。
pub mod vendor;

use std::path::Path;

/// legion 驱动栈预算（字节）。
///
/// 依据（2026-11 实测，目标 `ovm`，项目 `bitwise-ors-of-subarrays`）：
/// - HIR lowering 递归由表达式嵌套深度驱动，每层约 40 KiB 栈；`MAX_EXPRESSION_LOWERING_DEPTH`
///   允许 24 层，25 层嵌套的峰值约 1.05 MiB，正好越过 Windows 主线程默认 1 MiB。
/// - 同一程序 1 MiB 退出 `0xC00000FD`，1.1 MiB 起 exit 0；栈预算再翻倍（2/4/8/16 MiB）
///   结果不变，说明峰值由 guard 上限决定、不随程序规模增长。
/// - MIR/SSA/后端 lowering 尚未在这个项目上逼近上限，因此留 ~60× 余量。
///
/// 固定 64 MiB 只设在 worker 线程上，不改变主线程栈，也不要求调用方传魔数。
pub const LEGION_DRIVER_STACK_BYTES: usize = 64 * 1024 * 1024;

/// 在固定栈预算的 worker 线程上执行 legion 驱动入口。
///
/// `build`/`test`/`plan` 与 example 入口统一走这里，避免各入口散落不同的魔数栈大小。
pub fn run_with_driver_stack<T, F>(work: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let worker = std::thread::Builder::new()
        .name("legion-driver".into())
        .stack_size(LEGION_DRIVER_STACK_BYTES)
        .spawn(work)
        .expect("spawn legion driver thread");
    match worker.join() {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// CLI 日志用路径：尽量相对 cwd，或收敛到仓名相对片段，禁止原样打印本机绝对路径。
pub(crate) fn path_for_cli_log(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw.strip_prefix(r"\\?\").unwrap_or(raw.as_ref());
    let normalized = stripped.replace('\\', "/");
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_norm = cwd.to_string_lossy().replace('\\', "/");
        let cwd_trim = cwd_norm.trim_end_matches('/');
        if let Some(rel) = normalized.strip_prefix(cwd_trim) {
            let rel = rel.trim_start_matches('/');
            if !rel.is_empty() {
                return rel.to_string();
            }
        }
    }
    for marker in ["valkyrie.rs/", "valkyrie.v/", "nyar-vm.rs/", "leetcode.v/", "project-euler.v/"] {
        if let Some(i) = normalized.find(marker) {
            return normalized[i..].to_string();
        }
    }
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| normalized)
}
