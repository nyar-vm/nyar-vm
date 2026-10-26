use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use crate::legion::CanonicalTarget;

use crate::legion::workspace::manifest::{DependencySpec, ProjectManifest, WorkspaceManifest};

use super::types::{PlannerError, PlannerResult};

/// 收集工程源码：存在 `source/` 时取其中全部 `.v`，否则取根目录 `.v`。
pub fn collect_source_files(project_dir: &Path) -> PlannerResult<Vec<PathBuf>> {
    let project_dir = path_for_local_fs(project_dir);
    let source_dir = project_dir.join("source");
    // 标准包：`source/` 下全部 `.v`；可执行/导出入口由源码 `[main]` / `[export]` 标记。
    if source_dir.is_dir() {
        let mut files = Vec::new();
        collect_v_files(&source_dir, &mut files)?;
        files.sort();
        return Ok(files);
    }

    let mut files = Vec::new();
    collect_project_root_v_files(&project_dir, &mut files)?;
    files.sort();
    Ok(files)
}

/// 单脚本工程：项目根目录下的 `.v`（无 `source/` / `test/` 目录时使用）。
pub fn collect_project_root_v_files(project_dir: &Path, files: &mut Vec<PathBuf>) -> PlannerResult<()> {
    let project_dir = path_for_local_fs(project_dir);
    for entry in fs::read_dir(&project_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "v") {
            files.push(path);
        }
    }
    Ok(())
}

/// 项目是否采用单脚本布局（根目录 `.v`，无 `source/`）。
pub fn project_uses_single_script_layout(project_dir: &Path) -> bool {
    !project_dir.join("source").exists()
}

/// 收集项目 `source/` + `test/` 下的全部 `.v` 文件（测试编译专用）。
pub fn collect_test_build_sources(project_dir: &Path) -> PlannerResult<Vec<PathBuf>> {
    let mut files = collect_source_files(project_dir)?;
    let test_dir = project_dir.join("test");
    if test_dir.exists() {
        collect_v_files(&test_dir, &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// 仅收集项目 `test/` 下的 `.v` 文件；无 `source/` 布局时包含根目录 `.v`。
pub fn collect_test_v_files(project_dir: &Path) -> PlannerResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    let test_dir = project_dir.join("test");
    if test_dir.exists() {
        collect_v_files(&test_dir, &mut files)?;
    }
    else if project_uses_single_script_layout(project_dir) {
        collect_project_root_v_files(project_dir, &mut files)?;
    }
    files.sort();
    Ok(files)
}

/// 收集项目 `source/` 与 `test/` 中全部 `.v` 文件（覆盖率扫描，含 `compile_only/`）。
pub fn collect_project_v_files(project_dir: &Path) -> PlannerResult<Vec<PathBuf>> {
    let mut files = collect_source_files(project_dir)?;
    let test_dir = project_dir.join("test");
    if test_dir.exists() {
        collect_v_files_including_compile_only(&test_dir, &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

pub(crate) fn collect_v_files_including_compile_only(dir: &Path, files: &mut Vec<PathBuf>) -> PlannerResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_v_files_including_compile_only(&path, files)?;
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "v") {
            files.push(path);
        }
    }
    Ok(())
}

pub(crate) fn collect_v_files(dir: &Path, files: &mut Vec<PathBuf>) -> PlannerResult<()> {
    let dir = path_for_local_fs(dir);
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "compile_only") {
                continue;
            }
            collect_v_files(&path, files)?;
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "v") {
            files.push(path);
        }
    }
    Ok(())
}

pub(crate) fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let start_dir = search_start_dir(start);

    // Prefer the outermost `legions.von`. Nested subspaces (e.g. `projects/legion._`)
    // only declare local members; shared deps like `core` / `legion` / `std` live on the
    // parent super-workspace and are registered via nested member recursion.
    let mut outermost = None;
    let mut current = Some(start_dir.as_path());
    while let Some(dir) = current {
        if dir.join("legions.von").exists() {
            outermost = Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    outermost
}

/// 尽力规范化路径；`canonicalize` 失败时原样返回。
pub fn canonicalize_lossy(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Windows `canonicalize` 常返回 `\\?\` 扩展路径；部分 `read_dir` 调用方无法枚举，需还原为常规路径。
pub fn path_for_local_fs(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(stripped) = raw.strip_prefix(r"\\?\") { PathBuf::from(stripped) } else { path.to_path_buf() }
}

pub(crate) fn search_start_dir(start: &Path) -> PathBuf {
    if start.is_dir() {
        return canonicalize_lossy(start);
    }

    start.parent().map(canonicalize_lossy).unwrap_or_else(|| start.to_path_buf())
}

pub(crate) fn resolve_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(search_start_dir(start));
    while let Some(dir) = current {
        if dir.join("legion.von").exists() {
            return Some(dir);
        }
        current = dir.parent().map(Path::to_path_buf);
    }
    None
}

pub(crate) fn register_workspace_members(
    workspace_root: &Path,
    workspace_manifest: &WorkspaceManifest,
    projects: &mut BTreeMap<PathBuf, ProjectManifest>,
    projects_by_name: &mut BTreeMap<String, PathBuf>,
) -> PlannerResult<()> {
    for member in &workspace_manifest.members {
        let member_dir = canonicalize_lossy(&workspace_root.join(member));
        let nested_workspace_manifest_path = member_dir.join("legions.von");
        if nested_workspace_manifest_path.exists() {
            let nested_workspace_manifest = WorkspaceManifest::parse(&fs::read_to_string(&nested_workspace_manifest_path)?)?;
            register_workspace_members(&member_dir, &nested_workspace_manifest, projects, projects_by_name)?;
        }

        let manifest_path = member_dir.join("legion.von");
        if !manifest_path.exists() {
            continue;
        }

        // 跳过无法解析的 manifest，避免单个测试项目阻塞整个工作区发现。
        // 这类项目通常是语法特性测试，其 manifest 字段不遵循标准格式。
        let manifest = match ProjectManifest::parse(&fs::read_to_string(&manifest_path)?) {
            Ok(manifest) => manifest,
            Err(error) => {
                eprintln!("warning: 跳过无法解析的 manifest {}: {error}", manifest_path.display());
                continue;
            }
        };
        projects_by_name.insert(manifest.name.clone(), member_dir.clone());
        // 同时按 workspace 成员路径的 basename 建立别名索引。
        // 例如 `projects/core` 的 manifest.name 可能是 `valkyrie-core`，
        // 但 auto_link/dependencies 中引用的是逻辑名 `core`（即目录名）。
        if let Some(basename) = Path::new(member).file_name().and_then(|s| s.to_str()) {
            projects_by_name.entry(basename.to_string()).or_insert_with(|| member_dir.clone());
        }
        projects.insert(member_dir, manifest);
    }

    Ok(())
}

pub(crate) fn same_path(lhs: &Path, rhs: &Path) -> bool {
    normalize_path_for_lookup(lhs) == normalize_path_for_lookup(rhs)
}

fn normalize_path_for_lookup(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    if cfg!(windows) { stripped.to_ascii_lowercase() } else { stripped.to_string() }
}

pub(crate) fn project_visit_key(path: &Path) -> String {
    normalize_path_for_lookup(&path_for_local_fs(path))
}

/// 单次 build 允许纳入编译的最大 `.v` 文件数（含依赖闭包）。
pub(crate) const MAX_SOURCE_CLOSURE_FILES: usize = 512;
/// 语义源码组（依赖图节点）上限。
pub(crate) const MAX_SEMANTIC_GROUPS: usize = 64;
/// 语义依赖展开最大深度。
pub(crate) const MAX_SEMANTIC_GROUP_DEPTH: usize = 32;

pub(crate) fn manifest_supports_build_context(manifest: &ProjectManifest, target: &CanonicalTarget, requested_publish: &[String]) -> bool {
    if !manifest.build.is_empty() && !manifest.build.iter().any(|item| item.target == *target) {
        return false;
    }
    let Some(sdk_vendor) = &manifest.sdk_vendor
    else {
        return true;
    };
    if !sdk_vendor.targets.is_empty() {
        let target_matches = sdk_vendor
            .targets
            .iter()
            .any(|item| item == &target.to_string() || item.parse::<CanonicalTarget>().map(|parsed| parsed == *target).unwrap_or(false));
        if !target_matches {
            return false;
        }
    }
    if !sdk_vendor.publish.is_empty() && !requested_publish.is_empty() {
        let publish_matches = sdk_vendor.publish.iter().any(|item| requested_publish.iter().any(|requested| requested == item));
        if !publish_matches {
            return false;
        }
    }
    true
}

pub(crate) fn dependency_supports_source_context(
    dependency_name: &str,
    manifest: &ProjectManifest,
    target: &CanonicalTarget,
    requested_publish: &[String],
) -> bool {
    // `auto_link.core` 是语言语义依赖，不是一个按目标筛选的产物依赖。
    // core 的 manifest 只描述 seed 语义构建目标；若用同一过滤器匹配
    // Node/Wasm 或其他 backend target，会在进入 Compiler 前静默丢失 core。
    dependency_name == "core" || manifest_supports_build_context(manifest, target, requested_publish)
}

pub(crate) fn implicit_sdk_dependencies(publish: &[String]) -> Vec<&'static str> {
    let mut deps = Vec::new();
    if publish.iter().any(|item| item == "mini-game") {
        deps.push("tencent.wechat.sdk");
    }
    if publish.iter().any(|item| item == "mini-program") {
        deps.push("tencent.wechat.miniprogram.sdk");
    }
    if publish.iter().any(|item| item == "unity-player") {
        deps.push("unity.engine.sdk");
    }
    deps
}

/// 把发行版默认 SDK 并入根工程的候选依赖集合。
///
/// 排除工程自身（默认 SDK 不应依赖自己），并尊重显式 `Disabled`：显式禁用
/// 是装配层的收窄事实，不能被默认注入覆盖。是否真正进入闭包仍由后续
/// target / publish / sdk-vendor 过滤决定。
pub(crate) fn add_distribution_default_dependencies(manifest: &ProjectManifest, defaults: &BTreeSet<String>, names: &mut BTreeSet<String>) {
    for default_name in defaults {
        if default_name == &manifest.name {
            continue;
        }
        if matches!(manifest.dependencies.get(default_name), Some(DependencySpec::Disabled)) {
            continue;
        }
        names.insert(default_name.clone());
    }
}

fn manifest_supports_target(manifest: &ProjectManifest, target: &CanonicalTarget) -> bool {
    manifest_supports_build_context(manifest, target, &[])
}
