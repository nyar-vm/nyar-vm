use std::path::{Path, PathBuf};

use nyar::{CanonicalTarget, TargetResolver};

use crate::{PackageManagerError, Result};

/// 将 `publish[].target` 解析为规范目标身份及 `dist/{identity}` 产物目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPublishArtifact {
    /// 解析后的规范编译平台。
    pub canonical_target: CanonicalTarget,
    /// 用于 `dist/` 子目录名的完整身份（含宿主合同）。
    pub canonical_label: String,
    /// 已构建产物所在的绝对目录。
    pub artifact_dir: PathBuf,
}

/// Resolve `publish[].target` to a canonical identity and `dist/` subdirectory.
///
/// 运输渠道只决定 envelope 如何托管，不决定编译目标：`wasm` 产物发布到 npm/jsr
/// 仍是 `wasm` 目标，不得被渠道改写成 `node`。目标选择由构建方与发布声明决定。
/// `native` / `node` / `python` 占位符经统一的 [`TargetResolver`] 展开，目录身份
/// 使用展开后的完整身份（例如 Node-API 扩展带 `-node-api` 后缀）。
pub fn resolve_publish_artifact(package_path: &Path, build_target: &str) -> Result<ResolvedPublishArtifact> {
    let target_input = if build_target.trim().is_empty() { "wasm-node" } else { build_target.trim() };

    let resolver = TargetResolver::for_build_host()
        .map_err(|error| PackageManagerError::message(format!("无法探测构建宿主：{error}")))?;
    let resolved = resolver
        .resolve_label(target_input)
        .map_err(|error| PackageManagerError::message(format!("无法解析 publish target `{target_input}`：{error}")))?;
    let canonical_label = resolved.identity();
    let canonical_target = resolved.platform;
    let artifact_dir = package_path.join("dist").join(&canonical_label);
    if !artifact_dir.is_dir() {
        return Err(PackageManagerError::message(format!(
            "构建产物目录不存在：{}\n请先构建目标：build --target {}",
            artifact_dir.display(),
            publish_build_hint(target_input, &canonical_label)
        )));
    }

    Ok(ResolvedPublishArtifact { canonical_target, canonical_label, artifact_dir })
}

fn publish_build_hint(requested: &str, canonical: &str) -> String {
    if requested.eq_ignore_ascii_case(canonical) { requested.to_string() } else { format!("{requested}（或 {canonical}）") }
}
