//! Nyar package manager core library.
//!
//! 清单、工作区、lock 与依赖求解由 Boost 的 `package-resolve` 提供；本 crate 只保留产品侧的
//! 编排与自有渠道策略（Valhalla、官方 CLI 凭据、打包产物与许可证清单）。
#![deny(missing_docs)]

mod auth;
mod credentials;
mod error;
mod manager;
mod pack;
mod publish;
mod publish_target;
mod registry_sources;
mod scripts;
mod security;

pub use auth::{LoginResult, VendorAuthStore, VendorManager};
pub use credentials::{
    DiscoveredCredential, RegistryCredentialId, TokenDiscovery, discover_token, is_external_registry, sync_token_to_official_store,
};
pub use error::{PackageManagerError, Result};
pub use manager::{PackageInfo, PackageManager};
pub use pack::{
    PackMeta, PackResult, PackageIgnore, RegistryBinEntry, RegistryPackOptions, bin_name_from_logical, glob_match, pack,
    pack_registry_artifact, unpack,
};
pub use publish::PackagePublisher;
pub use publish_target::{ResolvedPublishArtifact, resolve_publish_artifact};
pub use registry_sources::RegistrySourceManager;
pub use scripts::{ScriptResult, ScriptRunner};
pub use security::{LicenseInfo, SecurityAudit, SecurityAuditResult, VulnerabilityReport, check_license};

pub use package_cache::PackageCache;
pub use package_resolve::{
    DependencyBucket, DependencyNode, DependencyResolver, DependencySpec, EntryAliasFn, LockEntry, LockFile, PackageError,
    PackageManifest, ProjectLayout, ProjectMode, PublishConfig, PublishTarget, SemanticVersion, WorkspaceManifest, YearlyVersion,
    is_registry_publish_type, satisfies_version_constraint,
};

pub use nyar_package_registry::{
    MockRegistry, Package, PublishOptions, PublishResult, PublisherKey, Registry, RegistryError, TokenVerifyResult, VersionBump,
};

/// 本产品的工程布局：清单 / lockfile 文件名与用户数据目录。
pub const PRODUCT_LAYOUT: ProjectLayout = ProjectLayout {
    package_manifest: "package.von",
    workspace_manifest: "workspace.von",
    ignore_file: ".packageignore",
    lockfile: "package-lock.von",
    home_dirname: ".nyar",
    home_env: "NYAR_HOME",
    token_env_vars: &[],
    entry_aliases: no_entry_aliases,
};

fn no_entry_aliases(_: &str) -> &'static [&'static str] {
    &[]
}
