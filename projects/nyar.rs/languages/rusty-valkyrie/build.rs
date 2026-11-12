//! 编译器构建身份：由 Git 修订或外部注入值生成，供工具链指纹与消费者报告读取。

use std::{env, io::Write, process::Command};

fn main() {
    // 源文件、HEAD 或分支 ref 变化都要重新生成身份；外部注入变量变化同样重跑。
    // 路径缺失时 Cargo 会按每次变化处理，仍能保证身份不过期。
    println!("cargo:rerun-if-changed=.");
    println!("cargo:rerun-if-changed=../../../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../../../.git/refs/heads/dev");
    // dirty 内容摘要覆盖整个仓库的 tracked 改动：编译器栈任何 crate 的源码变化
    // 都必须重跑本脚本，否则身份停留在旧摘要，不同构建共享同一身份。
    for dir in [
        "../../compilers/nyar/src",
        "../../compilers/nyar-analyzer/src",
        "../../compilers/nyar-emitter/src",
        "../../compilers/nyar-optimizer/src",
        "../../compilers/nyar-runner/src",
        "../../compilers/nyar-types/src",
        "../../compilers/nyar-workspace/src",
        "../../formats/odin-bytecode/src",
        "../../runtimes/odin-vm/src",
    ] {
        println!("cargo:rerun-if-changed={dir}");
    }
    println!("cargo:rerun-if-env-changed=NYAR_COMPILER_BUILD_ID");
    let build_id = env::var("NYAR_COMPILER_BUILD_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(git_build_id);
    println!("cargo:rustc-env=NYAR_COMPILER_BUILD_ID={build_id}");
}

/// 干净树取完整 rev；dirty 树把 tracked 改动的可复现内容摘要并入身份
/// （`git status --porcelain` + `git diff HEAD` 经 `git hash-object` 取 blob 摘要），
/// 不同 dirty 内容不再共享同一 `-dirty` 身份。全程只读，不写 index/refs/工作树。
///
/// fail-closed：Git 不可用、`git status` 读取失败、dirty 内容摘要（diff 或
/// hash-object）无法生成时，一律显式失败为 `unknown`——不得按 clean revision
/// 处理，也不得退化为裸 `-dirty` 后缀。`unknown` 会被权威 freshness 核心判为
/// 不可复用；原因经 `cargo:warning` 输出到构建日志。
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
    let status = Command::new("git").args(["status", "--porcelain", "--untracked-files=no"]).output();
    let Ok(status) = status
    else {
        warn("git status 无法执行：编译器构建身份显式失败为 `unknown`，不得按 clean revision 处理");
        return "unknown".to_string();
    };
    if !status.status.success() {
        warn("git status 非零退出：编译器构建身份显式失败为 `unknown`，不得按 clean revision 处理");
        return "unknown".to_string();
    }
    if status.stdout.is_empty() {
        return revision;
    }
    // dirty：身份必须绑定实际内容。`git diff HEAD` 同时覆盖已暂存与未暂存的
    // tracked 改动；status 清单补齐重命名/新增路径。diff 或 hash-object 失败时
    // 摘要无法绑定改动内容，显式失败为 `unknown`，绝不退化为裸 `-dirty` 后缀。
    let diff = Command::new("git").args(["diff", "HEAD"]).output();
    let Ok(diff) = diff
    else {
        warn("git diff HEAD 无法执行：dirty 内容摘要无法生成，编译器构建身份显式失败为 `unknown`");
        return "unknown".to_string();
    };
    if !diff.status.success() {
        warn("git diff HEAD 非零退出：dirty 内容摘要无法生成，编译器构建身份显式失败为 `unknown`");
        return "unknown".to_string();
    }
    let mut digest_input = status.stdout;
    digest_input.extend_from_slice(&diff.stdout);
    match git_blob_digest(&digest_input) {
        Some(digest) => format!("{revision}-dirty{}", &digest[..12]),
        None => {
            warn("git hash-object 失败：dirty 内容摘要无法生成，编译器构建身份显式失败为 `unknown`，拒绝退化为裸 `-dirty` 后缀");
            "unknown".to_string()
        }
    }
}

/// 构建脚本警告：身份失败原因进入构建日志，不静默吞掉。
fn warn(message: &str) {
    println!("cargo:warning={message}");
}

/// `git hash-object -t blob --stdin --no-filters`：对输入字节取 git blob 摘要（40 位十六进制）。
/// 不带 `-w`，只计算不写入对象库；同内容同摘要，Windows/CI 下可复现。
fn git_blob_digest(content: &[u8]) -> Option<String> {
    let mut child = Command::new("git")
        .args(["hash-object", "-t", "blob", "--stdin", "--no-filters"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.as_mut()?.write_all(content).ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let digest = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if digest.len() < 12 {
        return None;
    }
    Some(digest)
}
