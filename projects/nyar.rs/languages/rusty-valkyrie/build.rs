//! 编译器构建身份：由 Git 修订或外部注入值生成，供工具链指纹与消费者报告读取。

use std::{env, process::Command};

fn main() {
    // 源文件或 HEAD 变化都要重新生成身份；外部注入变量变化同样重跑。
    println!("cargo:rerun-if-changed=.");
    println!("cargo:rerun-if-env-changed=NYAR_COMPILER_BUILD_ID");
    let build_id = env::var("NYAR_COMPILER_BUILD_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(git_build_id);
    println!("cargo:rustc-env=NYAR_COMPILER_BUILD_ID={build_id}");
}

/// 取当前仓库修订；工作区有 tracked 改动时追加 `-dirty`。Git 不可用时回退 `unknown`。
fn git_build_id() -> String {
    let revision = Command::new("git").args(["rev-parse", "--short=12", "HEAD"]).output();
    let Ok(revision) = revision
    else {
        return "unknown".to_string();
    };
    if !revision.status.success() {
        return "unknown".to_string();
    }
    let revision = String::from_utf8_lossy(&revision.stdout).trim().to_string();
    if revision.is_empty() {
        return "unknown".to_string();
    }
    let dirty = Command::new("git").args(["status", "--porcelain", "--untracked-files=no"]).output();
    match dirty {
        Ok(status) if status.status.success() && !status.stdout.is_empty() => format!("{revision}-dirty"),
        _ => revision,
    }
}
