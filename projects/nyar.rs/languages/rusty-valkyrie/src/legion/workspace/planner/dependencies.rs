use std::{
    borrow::Cow,
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::legion::CanonicalTarget;

use crate::legion::workspace::{
    LOCAL_LEGION_CONFIG,
    manifest::{BuildTargetSpec, DependencySourcePreference, DependencySpec, LocalLegionConfig, ProjectManifest},
};

use super::{
    sources::canonicalize_lossy,
    types::{PlannerError, PlannerResult, WorkspaceResolver},
};

/// 合并本地 `.config/legion/legions.von` 依赖覆盖（越靠近项目的配置优先级越高）。
pub(crate) fn merged_local_dependency_overrides(project_dir: &Path) -> BTreeMap<String, (PathBuf, DependencySpec)> {
    let mut merged = BTreeMap::new();

    if let Some((anchor, config)) = load_local_legion_config(user_local_config_path()) {
        for (name, spec) in config.dependencies {
            merged.insert(name, (anchor.clone(), spec));
        }
    }

    let mut chain = Vec::new();
    let mut current = Some(canonicalize_lossy(project_dir));
    while let Some(dir) = current {
        let config_path = dir.join(LOCAL_LEGION_CONFIG);
        if config_path.is_file() {
            if let Ok(source) = fs::read_to_string(&config_path) {
                if let Ok(config) = LocalLegionConfig::parse(&source) {
                    chain.push((local_config_anchor(&config_path), config.dependencies));
                }
            }
        }
        current = dir.parent().map(Path::to_path_buf);
    }

    for (anchor, deps) in chain.into_iter().rev() {
        for (name, spec) in deps {
            merged.insert(name, (anchor.clone(), spec));
        }
    }

    merged
}

pub(crate) fn effective_dependency_spec<'a>(
    dependency_name: &str,
    declared: Option<&'a DependencySpec>,
    overrides: &BTreeMap<String, (PathBuf, DependencySpec)>,
    project_manifest_dir: &Path,
) -> (Option<Cow<'a, DependencySpec>>, PathBuf) {
    if let Some((anchor, spec)) = overrides.get(dependency_name) {
        return (Some(Cow::Owned(spec.clone())), anchor.clone());
    }
    (declared.map(Cow::Borrowed), project_manifest_dir.to_path_buf())
}

fn user_local_config_path() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").ok().or_else(|| std::env::var("HOME").ok()).map(PathBuf::from)?;
    let path = home.join(".config").join("legion").join("legions.von");
    path.is_file().then_some(path)
}

fn local_config_anchor(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .and_then(|legion| legion.parent())
        .and_then(|config| config.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| config_path.to_path_buf())
}

fn load_local_legion_config(config_path: Option<PathBuf>) -> Option<(PathBuf, LocalLegionConfig)> {
    let config_path = config_path?;
    let source = fs::read_to_string(&config_path).ok()?;
    let config = LocalLegionConfig::parse(&source).ok()?;
    Some((local_config_anchor(&config_path), config))
}

pub(crate) enum ResolvedDependencySource {
    Local(PathBuf),
    Registry { version: String, registry: String },
    MissingImplicit,
}

pub(crate) fn resolve_dependency_source(
    workspace: &WorkspaceResolver,
    project_manifest_dir: &Path,
    path_base: &Path,
    dependency_spec: Option<&DependencySpec>,
    local_manifest_dir: Option<PathBuf>,
    allow_missing_when_auto: bool,
    project_name: &str,
    dependency_name: &str,
) -> PlannerResult<ResolvedDependencySource> {
    let source_preference = dependency_spec.map(DependencySpec::source_preference).unwrap_or(DependencySourcePreference::Auto);
    let version_hint = dependency_spec.and_then(DependencySpec::version_hint);
    let registry_hint = dependency_spec.and_then(DependencySpec::registry_hint).unwrap_or("npm");

    match source_preference {
        DependencySourcePreference::Path => {
            let raw_path = dependency_spec.and_then(DependencySpec::path_hint).ok_or_else(|| PlannerError::PathDependencyMissingPath {
                project: project_name.to_string(),
                dependency: dependency_name.to_string(),
            })?;
            let manifest_dir = resolve_local_dependency_path(path_base, raw_path);
            if !manifest_dir.join("legion.von").is_file() {
                return Err(PlannerError::PathDependencyMissingManifest {
                    project: project_name.to_string(),
                    dependency: dependency_name.to_string(),
                    manifest_dir,
                });
            }
            Ok(ResolvedDependencySource::Local(manifest_dir))
        }
        DependencySourcePreference::Git => {
            let git_url = dependency_spec.and_then(DependencySpec::git_url).ok_or_else(|| PlannerError::GitDependencyMissingUrl {
                project: project_name.to_string(),
                dependency: dependency_name.to_string(),
            })?;
            let git_ref = dependency_spec.and_then(DependencySpec::git_ref_hint);
            let subpath = dependency_spec.and_then(DependencySpec::path_hint);
            let manifest_dir = workspace.ensure_git_dependency(git_url, git_ref, subpath, project_name, dependency_name)?;
            Ok(ResolvedDependencySource::Local(manifest_dir))
        }
        DependencySourcePreference::Workspace => local_manifest_dir.map(ResolvedDependencySource::Local).ok_or_else(|| {
            PlannerError::ForcedWorkspaceDependencyMissing { project: project_name.to_string(), dependency: dependency_name.to_string() }
        }),
        DependencySourcePreference::Registry => {
            let Some(version) = version_hint
            else {
                return Err(PlannerError::RegistryDependencyMissingVersion {
                    project: project_name.to_string(),
                    dependency: dependency_name.to_string(),
                });
            };
            Ok(ResolvedDependencySource::Registry { version: version.to_string(), registry: registry_hint.to_string() })
        }
        DependencySourcePreference::Auto => {
            if let Some(local_manifest_dir) = local_manifest_dir {
                return Ok(ResolvedDependencySource::Local(local_manifest_dir));
            }
            if let Some(spec) = dependency_spec {
                if spec.git_url().is_some() {
                    return resolve_dependency_source(
                        workspace,
                        project_manifest_dir,
                        path_base,
                        Some(&DependencySpec::Detailed {
                            version: spec.version_hint().map(str::to_string),
                            path: spec.path_hint().map(str::to_string),
                            abi: None,
                            source: Some("git".to_string()),
                            registry: None,
                            git: spec.git_url().map(str::to_string),
                            git_ref: spec.git_ref_hint().map(str::to_string),
                        }),
                        local_manifest_dir,
                        allow_missing_when_auto,
                        project_name,
                        dependency_name,
                    );
                }
                if spec.path_hint().is_some() {
                    return resolve_dependency_source(
                        workspace,
                        project_manifest_dir,
                        path_base,
                        Some(&DependencySpec::Detailed {
                            version: None,
                            path: spec.path_hint().map(str::to_string),
                            abi: None,
                            source: Some("path".to_string()),
                            registry: None,
                            git: None,
                            git_ref: None,
                        }),
                        local_manifest_dir,
                        allow_missing_when_auto,
                        project_name,
                        dependency_name,
                    );
                }
            }
            if let Some(version) = version_hint {
                if version == "workspace" {
                    return local_manifest_dir.map(ResolvedDependencySource::Local).ok_or_else(|| {
                        PlannerError::ForcedWorkspaceDependencyMissing {
                            project: project_name.to_string(),
                            dependency: dependency_name.to_string(),
                        }
                    });
                }
                return Ok(ResolvedDependencySource::Registry { version: version.to_string(), registry: registry_hint.to_string() });
            }
            if allow_missing_when_auto {
                return Ok(ResolvedDependencySource::MissingImplicit);
            }
            Err(PlannerError::MissingDependency { project: project_name.to_string(), dependency: dependency_name.to_string() })
        }
    }
}

fn resolve_local_dependency_path(project_manifest_dir: &Path, raw_path: &str) -> PathBuf {
    let candidate = Path::new(raw_path);
    if candidate.is_absolute() { candidate.to_path_buf() } else { project_manifest_dir.join(candidate) }
}

impl WorkspaceResolver {
    fn git_vendor_root(&self, git_url: &str, git_ref: &str) -> PathBuf {
        let slug = git_cache_slug(git_url, git_ref);
        self.root_dir.join("vendors").join("git").join(slug)
    }

    fn ensure_git_dependency(
        &self,
        git_url: &str,
        git_ref: Option<&str>,
        subpath: Option<&str>,
        project_name: &str,
        dependency_name: &str,
    ) -> PlannerResult<PathBuf> {
        let git_ref = git_ref.unwrap_or("main");
        let vendor_root = self.git_vendor_root(git_url, git_ref);
        let manifest_dir = match subpath {
            Some(path) => vendor_root.join(path),
            None => vendor_root.clone(),
        };

        if manifest_dir.join("legion.von").is_file() {
            return Ok(manifest_dir);
        }

        if !vendor_root.join(".git").is_dir() {
            if vendor_root.exists() {
                fs::remove_dir_all(&vendor_root)?;
            }
            if let Some(parent) = vendor_root.parent() {
                fs::create_dir_all(parent)?;
            }
            run_git(
                project_name,
                dependency_name,
                git_url,
                Command::new("git").args(["clone", "--depth", "1", "--branch", git_ref, git_url]).arg(&vendor_root),
            )?;
        }
        else {
            run_git(
                project_name,
                dependency_name,
                git_url,
                Command::new("git").current_dir(&vendor_root).args(["fetch", "origin", git_ref, "--depth", "1"]),
            )?;
            run_git(project_name, dependency_name, git_url, Command::new("git").current_dir(&vendor_root).args(["checkout", "FETCH_HEAD"]))?;
        }

        if !manifest_dir.join("legion.von").is_file() {
            return Err(PlannerError::GitDependencyMissingManifest {
                project: project_name.to_string(),
                dependency: dependency_name.to_string(),
                git_url: git_url.to_string(),
                git_ref: git_ref.to_string(),
                manifest_dir,
            });
        }

        Ok(manifest_dir)
    }
}

fn git_cache_slug(git_url: &str, git_ref: &str) -> String {
    let mut slug = String::new();
    for ch in git_url.chars().chain(git_ref.chars()) {
        slug.push(if ch.is_ascii_alphanumeric() { ch } else { '_' });
    }
    if slug.len() > 96 {
        slug.truncate(96);
    }
    if slug.is_empty() {
        slug.push('x');
    }
    slug
}

fn run_git(project: &str, dependency: &str, git_url: &str, command: &mut Command) -> PlannerResult<()> {
    let output = command.output().map_err(PlannerError::Io)?;
    if output.status.success() {
        return Ok(());
    }
    Err(PlannerError::GitDependencyCheckoutFailed {
        project: project.to_string(),
        dependency: dependency.to_string(),
        git_url: git_url.to_string(),
        reason: format_git_failure(command.get_program(), &output.stderr),
    })
}

fn format_git_failure(program: &OsStr, stderr: &[u8]) -> String {
    let stderr_text = String::from_utf8_lossy(stderr);
    let text = stderr_text.trim();
    if text.is_empty() { format!("`{}` exited with failure", program.to_string_lossy()) } else { text.to_string() }
}

/// 将字符串解析为规范化目标平台。
pub fn canonical_target(target: &str) -> std::result::Result<CanonicalTarget, crate::legion::CanonicalTargetParseError> {
    target.parse()
}

pub(crate) fn default_dist_output_dir(project_dir: &Path, target: &CanonicalTarget) -> PathBuf {
    project_dir.join("dist").join(target.as_canonical_str())
}

pub(crate) fn select_build_target(manifest: &ProjectManifest, target: &CanonicalTarget) -> Option<BuildTargetSpec> {
    manifest.build.iter().find(|item| item.target == *target).cloned().or_else(|| synthesize_cli_build_target(manifest, target))
}

/// CLI 显式 `--target` 未写入 `legion.von` 时，为 library 工程合成默认 build 项（单文件库工程等）。
fn synthesize_cli_build_target(manifest: &ProjectManifest, target: &CanonicalTarget) -> Option<BuildTargetSpec> {
    if manifest.artifact != crate::legion::workspace::manifest::ProjectArtifactKind::Library {
        return None;
    }
    Some(BuildTargetSpec { target: *target, ..BuildTargetSpec::default() })
}
