use crate::compiler::semantic::closure::DeclarationKey;
use nyar::CanonicalTarget;

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    path::PathBuf,
};

use crate::legion::workspace::manifest::{BuildTargetSpec, ManifestError, ProjectManifest, WorkspaceManifest};

/// Oak `cfg` attribute 上的一个键值条件（`cfg(key = "value")`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfgPredicate {
    /// 条件键（如 `target_vendor`）。
    pub key: String,
    /// 条件值（字符串字面量求值结果）。
    pub value: String,
}

/// 单次构建请求：工程目录、目标平台与可选输出目录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildRequest {
    /// 请求的工程根目录。
    pub project_dir: PathBuf,
    /// 目标平台。
    pub target: CanonicalTarget,
    /// 自定义输出目录；缺省时按目标推导默认 dist 目录。
    pub output_dir: Option<PathBuf>,
}

/// 构建计划中记录的一项已解析依赖。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedDependency {
    /// 依赖名称。
    pub name: String,
    /// 依赖 manifest 所在目录。
    pub manifest_dir: PathBuf,
}

/// One isolated frontend compilation unit. Dependencies are represented by
/// their exported semantic contracts, never by concatenating their source
/// text into this group's parser input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedSemanticSourceGroup {
    /// Resolver-assigned dependency identity used for semantic export lookup.
    pub dependency_key: String,
    /// 源码组名称（依赖包名或入口工程名）。
    pub name: String,
    /// 该源码组 manifest 所在目录。
    pub manifest_dir: PathBuf,
    /// 属于该源码组的源码文件列表。
    pub source_files: Vec<PathBuf>,
    /// 该源码组直接依赖的依赖键列表。
    pub direct_dependencies: Vec<String>,
    /// 当前构建入口工程组；依赖包 inventory 可容忍不可达文件的 parse 失败。
    pub is_entry: bool,
}

/// 计划中的宿主契约：源码声明的宿主接口、冻结声明身份及其位置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedHostContract {
    /// 契约限定名（`namespace` + 拥有者 + 声明名，统一 `::` 分隔）。
    pub id: String,
    /// 契约的冻结声明身份；Compiler 只接受结构化身份，不接受限定名字符串。
    pub key: DeclarationKey,
    /// 声明该契约的源码文件。
    pub source_file: PathBuf,
    /// 声明所在行号。
    pub line: usize,
    /// 契约在源码中带 Valkyrie 实现体；无 provider 时可本地降级，不要求宿主绑定。
    #[serde(default)]
    pub has_local_fallback_body: bool,
}

/// 计划中的宿主契约提供者候选：把契约绑定到宿主符号。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedHostProvider {
    /// 被实现的宿主契约标识（保留源码属性中的书写形式）。
    pub contract: String,
    /// 被引用契约的冻结声明身份；候选被选中时由 Resolver 填充。
    pub contract_key: Option<DeclarationKey>,
    /// 宿主侧绑定的符号名（提供者声明限定名）。
    pub symbol: String,
    /// 提供者声明的冻结身份。
    pub key: DeclarationKey,
    /// 声明该提供者的源码文件。
    pub source_file: PathBuf,
    /// 声明所在行号。
    pub line: usize,
    /// 由 `[js]` / `[js_builtin]` / `[wasm]` 等推导出的 capability 需求。
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    /// 声明上的外部绑定 attribute 原文（规划与诊断用）。
    #[serde(default)]
    pub foreign_bindings: Vec<String>,
    /// 附着于该 provider 的 `cfg` 条件（Planner 求值；前端 cfg lowering 未落地前用于消歧）。
    #[serde(default)]
    pub cfg_predicates: Vec<CfgPredicate>,
}

/// 单个工程的构建计划：清单位置、源码闭包、语义源码组与宿主契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedProject {
    /// 工程名称。
    pub name: String,
    /// 工程 manifest 所在目录。
    pub manifest_dir: PathBuf,
    /// 工程 manifest 文件路径。
    pub manifest_path: PathBuf,
    /// 该工程完整的源码输入闭包（用于哈希、宿主 inventory 与运行摘要）。
    pub source_files: Vec<PathBuf>,
    /// Dependency-first semantic compilation groups. `source_files` remains
    /// the complete input closure for hashes, host inventory and run digests.
    pub semantic_source_groups: Vec<PlannedSemanticSourceGroup>,
    /// 工程声明的宿主契约列表。
    pub host_contracts: Vec<PlannedHostContract>,
    /// 宿主契约提供者候选列表。
    pub host_provider_candidates: Vec<PlannedHostProvider>,
    /// 已选定的宿主契约提供者。
    pub selected_host_providers: Vec<PlannedHostProvider>,
    /// 匹配到的构建目标配置。
    pub build_target: BuildTargetSpec,
    /// 产物类型（binary / library）。
    pub artifact_kind: crate::legion::workspace::manifest::ProjectArtifactKind,
    /// 已解析的依赖列表。
    pub dependencies: Vec<PlannedDependency>,
}

/// 完整构建计划：workspace 根、目标工程与输出目录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildPlan {
    /// workspace 根目录。
    pub workspace_root: PathBuf,
    /// 目标工程的构建计划。
    pub project: PlannedProject,
    /// 产物输出目录。
    pub output_dir: PathBuf,
}

/// 工程解析模式：workspace 成员、独立包或单脚本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectResolutionMode {
    /// 通过 workspace `legions.von` 解析。
    Workspace,
    /// 独立包（仅有 `legion.von`）。
    Package,
    /// 内嵌清单的单脚本 `.v`。
    Script,
}

/// workspace 解析器：缓存已发现的工程清单与单脚本上下文。
#[derive(Debug)]
pub struct WorkspaceResolver {
    /// workspace 根目录。
    pub root_dir: PathBuf,
    /// workspace 根清单；独立包模式下为 `None`。
    pub workspace_manifest: Option<WorkspaceManifest>,
    pub(crate) projects: BTreeMap<PathBuf, ProjectManifest>,
    pub(crate) projects_by_name: BTreeMap<String, PathBuf>,
    /// `legion build solution.v` — 内嵌清单的单脚本模式。
    pub single_script: Option<crate::legion::workspace::script::SingleScriptContext>,
}

/// `WorkspaceResolver` / build plan 操作的 `Result` 别名（禁止 `Result<T, String>`）。
pub type PlannerResult<T> = std::result::Result<T, PlannerError>;

/// planner 在解析 workspace / 依赖并生成构建计划时产生的错误。
#[derive(Debug)]
pub enum PlannerError {
    /// 底层 I/O 错误。
    Io(std::io::Error),
    /// 清单解析错误。
    Manifest(ManifestError),
    /// 未能从起始路径定位 `legions.von`。
    MissingWorkspace(PathBuf),
    /// 未能定位 `legion.von`。
    MissingProjectManifest(PathBuf),
    /// CLI/N-API 传入的路径不是可识别的项目或脚本。
    InvalidProjectInput {
        /// 用户传入的路径。
        path: PathBuf,
    },
    /// 构建计划下没有任何源码文件。
    NoSourceFiles,
    /// Compiler / Emitter 失败（待 `nyar-emitter` 去掉 miette 后改为结构化 `E`）。
    CompileFailed {
        /// 驱动层返回的可显示原因。
        message: String,
    },
    /// 工程未声明请求的构建目标。
    MissingBuildTarget {
        /// 工程名称。
        project: String,
        /// 请求的目标平台。
        target: CanonicalTarget,
    },
    /// 无法解析某依赖。
    MissingDependency {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
    },
    /// 依赖要求使用 workspace 成员，但未找到对应成员。
    ForcedWorkspaceDependencyMissing {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
    },
    /// registry 依赖缺少版本号。
    RegistryDependencyMissingVersion {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
    },
    /// registry 依赖安装失败。
    RegistryDependencyInstallFailed {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
        /// 依赖版本。
        version: String,
        /// registry 名称。
        registry: String,
        /// 失败原因。
        reason: String,
    },
    /// path 依赖缺少路径字段。
    PathDependencyMissingPath {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
    },
    /// path 依赖目录下找不到 manifest。
    PathDependencyMissingManifest {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
        /// 依赖 manifest 目录。
        manifest_dir: PathBuf,
    },
    /// git 依赖缺少仓库地址。
    GitDependencyMissingUrl {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
    },
    /// git 依赖目录下找不到 manifest。
    GitDependencyMissingManifest {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
        /// git 仓库地址。
        git_url: String,
        /// git 引用。
        git_ref: String,
        /// 检出后的 manifest 目录。
        manifest_dir: PathBuf,
    },
    /// git 依赖检出失败。
    GitDependencyCheckoutFailed {
        /// 工程名称。
        project: String,
        /// 依赖名称。
        dependency: String,
        /// git 仓库地址。
        git_url: String,
        /// 失败原因。
        reason: String,
    },
    /// 宿主契约提供者声明了未知的契约。
    UnknownHostProviderContract {
        /// 提供者名称。
        provider: String,
        /// 契约标识。
        contract: String,
        /// 声明该提供者的源码文件。
        source_file: PathBuf,
        /// 声明所在行号。
        line: usize,
    },
    /// 多个提供者实现同一宿主契约，产生冲突。
    ConflictingHostProviders {
        /// 冲突的契约标识。
        contract: String,
        /// 冲突的提供者名称列表。
        providers: Vec<String>,
    },
    /// 契约在闭包内被需求，但没有任何 provider 满足当前 target capability 集合。
    HostProviderCapabilityMissing {
        /// 宿主契约标识。
        contract: String,
        /// 可见 provider 及其所需 capability。
        providers: Vec<String>,
        /// 当前 target 可用 capability。
        available_capabilities: Vec<String>,
    },
    /// 单脚本模式错误。
    Script(crate::legion::workspace::script::ScriptError),
    /// 防止依赖图或源码闭包失控（死循环 / 指数膨胀 / OOM）。
    ResourceLimitExceeded {
        /// 触发的上限类别。
        kind: &'static str,
        /// 允许的上限值。
        limit: usize,
        /// 实际达到的值。
        actual: usize,
    },
}

impl Display for PlannerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => Display::fmt(error, f),
            Self::Manifest(error) => Display::fmt(error, f),
            Self::InvalidProjectInput { path } => {
                write!(f, "找不到项目或脚本 `{}`（需要 `legion.von`、内嵌 `# ```legion` 的 `.v`，或 `solution.v`）", path.display())
            }
            Self::NoSourceFiles => f.write_str("没有找到任何源码文件"),
            Self::CompileFailed { message } => f.write_str(message),
            Self::MissingWorkspace(path) => write!(f, "cannot locate `legions.von` from {}", path.display()),
            Self::MissingProjectManifest(path) => write!(f, "cannot locate `legion.von` in {}", path.display()),
            Self::MissingBuildTarget { project, target } => {
                write!(f, "project '{}' does not declare build target '{}'", project, target)
            }
            Self::MissingDependency { project, dependency } => {
                write!(f, "project '{}' cannot resolve dependency '{}'", project, dependency)
            }
            Self::ForcedWorkspaceDependencyMissing { project, dependency } => {
                write!(f, "project '{}' requires workspace dependency '{}', but no workspace member was found", project, dependency)
            }
            Self::RegistryDependencyMissingVersion { project, dependency } => {
                write!(f, "project '{}' sets dependency '{}' source=registry but no version is provided", project, dependency)
            }
            Self::RegistryDependencyInstallFailed { project, dependency, version, registry, reason } => {
                write!(
                    f,
                    "project '{}' dependency '{}' failed to install from registry '{}' version '{}': {}",
                    project, dependency, registry, version, reason
                )
            }
            Self::PathDependencyMissingPath { project, dependency } => {
                write!(f, "project '{}' dependency '{}' uses source=path but no path is provided", project, dependency)
            }
            Self::PathDependencyMissingManifest { project, dependency, manifest_dir } => {
                write!(f, "project '{}' dependency '{}' path does not contain `legion.von` at {}", project, dependency, manifest_dir.display())
            }
            Self::GitDependencyMissingUrl { project, dependency } => {
                write!(f, "project '{}' dependency '{}' uses source=git but no git url is provided", project, dependency)
            }
            Self::GitDependencyMissingManifest { project, dependency, git_url, git_ref, manifest_dir } => {
                write!(
                    f,
                    "project '{}' dependency '{}' git checkout `{}`@{} has no `legion.von` at {}",
                    project,
                    dependency,
                    git_url,
                    git_ref,
                    manifest_dir.display()
                )
            }
            Self::GitDependencyCheckoutFailed { project, dependency, git_url, reason } => {
                write!(f, "project '{}' dependency '{}' failed to checkout git `{}`: {}", project, dependency, git_url, reason)
            }
            Self::UnknownHostProviderContract { provider, contract, source_file, line } => {
                write!(f, "host provider '{}' references unknown host contract '{}' at {}:{}", provider, contract, source_file.display(), line)
            }
            Self::ConflictingHostProviders { contract, providers } => {
                write!(f, "host contract '{}' has multiple visible providers: {}", contract, providers.join(", "))
            }
            Self::HostProviderCapabilityMissing { contract, providers, available_capabilities } => {
                write!(
                    f,
                    "host contract '{}' has no provider compatible with target capabilities [{}]; candidates: {}",
                    contract,
                    available_capabilities.join(", "),
                    providers.join(", ")
                )
            }
            Self::Script(error) => Display::fmt(error, f),
            Self::ResourceLimitExceeded { kind, limit, actual } => {
                write!(f, "planner {kind} exceeded limit {limit} (actual {actual})")
            }
        }
    }
}

impl std::error::Error for PlannerError {}

impl From<std::io::Error> for PlannerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ManifestError> for PlannerError {
    fn from(value: ManifestError) -> Self {
        Self::Manifest(value)
    }
}

impl From<crate::legion::workspace::script::ScriptError> for PlannerError {
    fn from(value: crate::legion::workspace::script::ScriptError) -> Self {
        Self::Script(value)
    }
}
