//! On-disk project layout names for package / workspace discovery.
//!
//! Product CLIs inject their own filenames and home paths via [`ProjectLayout`].
//! This crate stays product-identity free: no hard-coded product manifest names,
//! lockfiles, or home directories.

use std::path::{Path, PathBuf};

/// Resolves alternate physical entry names when packing / locating run-contract artifacts.
///
/// Products may inject bootstrap aliases (e.g. historical mangled names → canonical).
/// The neutral layout returns no aliases.
pub type EntryAliasFn = fn(&str) -> &'static [&'static str];

/// Filenames and user-data locations used when discovering and persisting
/// package-manager projects.
#[derive(Debug, Clone, Copy)]
pub struct ProjectLayout {
    /// Per-package manifest file name (e.g. `package.von`).
    pub package_manifest: &'static str,
    /// Workspace manifest file name (e.g. `workspace.von`).
    pub workspace_manifest: &'static str,
    /// Optional ignore file for packing (e.g. `.packageignore`).
    pub ignore_file: &'static str,
    /// Lockfile name under the project root (e.g. `package-lock.von`).
    pub lockfile: &'static str,
    /// Product data directory under the user home (e.g. `.nyar`).
    pub home_dirname: &'static str,
    /// Env var that overrides the user home root used with [`Self::home_dirname`].
    pub home_env: &'static str,
    /// Product-specific token env vars consulted after `{REGISTRY}_TOKEN`.
    pub token_env_vars: &'static [&'static str],
    /// Product-supplied physical-entry aliases tried before the contract name itself.
    pub entry_aliases: EntryAliasFn,
}

fn no_entry_aliases(_: &str) -> &'static [&'static str] {
    &[]
}

impl ProjectLayout {
    /// Neutral default layout with no product branding.
    pub const fn neutral() -> Self {
        Self {
            package_manifest: "package.von",
            workspace_manifest: "workspace.von",
            ignore_file: ".packageignore",
            lockfile: "package-lock.von",
            home_dirname: ".nyar",
            home_env: "NYAR_HOME",
            token_env_vars: &[],
            entry_aliases: no_entry_aliases,
        }
    }

    /// Path to the package manifest under `directory`.
    pub fn package_manifest_path(self, directory: impl AsRef<Path>) -> PathBuf {
        directory.as_ref().join(self.package_manifest)
    }

    /// Path to the workspace manifest under `directory`.
    pub fn workspace_manifest_path(self, directory: impl AsRef<Path>) -> PathBuf {
        directory.as_ref().join(self.workspace_manifest)
    }

    /// Path to the pack ignore file under `directory`.
    pub fn ignore_file_path(self, directory: impl AsRef<Path>) -> PathBuf {
        directory.as_ref().join(self.ignore_file)
    }

    /// Path to the lockfile under `directory`.
    pub fn lockfile_path(self, directory: impl AsRef<Path>) -> PathBuf {
        directory.as_ref().join(self.lockfile)
    }

    /// Resolve the user home root (`home_env`, then `HOME` / `USERPROFILE`).
    pub fn resolve_user_home(self) -> PathBuf {
        std::env::var_os(self.home_env)
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Product data directory (`$home/{home_dirname}`).
    pub fn product_data_dir(self) -> PathBuf {
        self.resolve_user_home().join(self.home_dirname)
    }

    /// Content-addressable package / security cache root.
    pub fn cache_dir(self) -> PathBuf {
        self.product_data_dir().join("cache")
    }

    /// Vendor auth store path (`auth.von` under the product data dir).
    pub fn auth_store_path(self) -> PathBuf {
        self.product_data_dir().join("auth.von")
    }

    /// Registry endpoint config path.
    pub fn registry_sources_path(self) -> PathBuf {
        self.product_data_dir().join("registry-sources.von")
    }
}

impl Default for ProjectLayout {
    fn default() -> Self {
        Self::neutral()
    }
}
