use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use oak_von::from_str;
use serde::{Deserialize, Serialize};

use crate::{PackageManagerError, PackageManifest, ProjectLayout, Result};

/// Workspace 清单（成员列表）；磁盘文件名由 [`ProjectLayout`] 指定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkspaceManifest {
    /// Workspace 名称。
    #[serde(default)]
    pub name: String,
    /// Workspace 统一版本（成员可继承）。
    #[serde(default)]
    pub version: String,
    /// 成员目录相对路径列表。
    #[serde(default)]
    pub members: Vec<String>,
}

impl WorkspaceManifest {
    /// 返回 `directory` 下 workspace 清单文件路径。
    pub fn path_in(directory: impl AsRef<Path>, layout: ProjectLayout) -> PathBuf {
        layout.workspace_manifest_path(directory)
    }

    /// 判断 workspace 清单是否存在于 `directory`。
    pub fn exists(directory: impl AsRef<Path>, layout: ProjectLayout) -> bool {
        Self::path_in(directory, layout).is_file()
    }

    /// 从磁盘加载 workspace 清单。
    pub fn load(directory: impl AsRef<Path>, layout: ProjectLayout) -> Result<Self> {
        let source = std::fs::read_to_string(Self::path_in(directory, layout))?;
        Ok(from_str(&source)?)
    }

    /// Resolve member paths relative to the workspace root (non-recursive).
    pub fn member_paths(&self, root: impl AsRef<Path>) -> Vec<PathBuf> {
        let root = root.as_ref();
        self.members.iter().map(|member| root.join(member)).collect()
    }

    /// Recursively enumerate member directories that contain a package manifest.
    pub fn enumerate_member_dirs(&self, root: impl AsRef<Path>, layout: ProjectLayout) -> Result<Vec<PathBuf>> {
        let mut members = Vec::new();
        self.collect_member_dirs(root.as_ref(), layout, &mut members)?;
        Ok(members)
    }

    fn collect_member_dirs(&self, workspace_root: &Path, layout: ProjectLayout, members: &mut Vec<PathBuf>) -> Result<()> {
        for member in &self.members {
            let member_dir = workspace_root.join(member);
            if Self::exists(&member_dir, layout) {
                let nested_workspace_manifest = Self::load(&member_dir, layout)?;
                nested_workspace_manifest.collect_member_dirs(&member_dir, layout, members)?;
            }
            if PackageManifest::exists(&member_dir, layout) {
                members.push(member_dir);
            }
        }
        Ok(())
    }

    /// Map package name -> member directory with a package manifest (recursive).
    pub fn member_packages(&self, root: impl AsRef<Path>, layout: ProjectLayout) -> Result<BTreeMap<String, PathBuf>> {
        let mut map = BTreeMap::new();
        for path in self.enumerate_member_dirs(root, layout)? {
            let manifest = PackageManifest::load(&path, layout)?;
            map.insert(manifest.name.clone(), path.clone());
            if let Some(basename) = path.file_name().and_then(|name| name.to_str()) {
                map.entry(basename.to_string()).or_insert(path);
            }
        }
        Ok(map)
    }
}

/// 从目录结构发现包管理器项目模式。
#[derive(Debug, Clone)]
pub enum ProjectMode {
    /// 根目录含 workspace 清单；`manifest` 为根包清单（若存在）。
    Workspace {
        /// 项目根目录。
        root: PathBuf,
        /// 已加载的 workspace 清单。
        workspace: WorkspaceManifest,
        /// 根目录上的可选包清单。
        manifest: Option<PackageManifest>,
    },
    /// 单包项目：根目录仅有包清单。
    Package {
        /// 项目根目录。
        root: PathBuf,
        /// 包清单。
        manifest: PackageManifest,
    },
    /// 无 workspace / 包清单的裸目录。
    Standalone {
        /// 目录路径。
        root: PathBuf,
    },
}

impl ProjectMode {
    /// 探测 `directory` 是 workspace、单包还是 standalone。
    pub fn discover(directory: impl AsRef<Path>, layout: ProjectLayout) -> Result<Self> {
        let root = directory.as_ref().to_path_buf();
        if WorkspaceManifest::exists(&root, layout) {
            let workspace = WorkspaceManifest::load(&root, layout)?;
            let manifest = if PackageManifest::exists(&root, layout) { Some(PackageManifest::load(&root, layout)?) } else { None };
            return Ok(Self::Workspace { root, workspace, manifest });
        }
        if PackageManifest::exists(&root, layout) {
            return Ok(Self::Package { manifest: PackageManifest::load(&root, layout)?, root });
        }
        Ok(Self::Standalone { root })
    }

    /// 项目根目录路径。
    pub fn root(&self) -> &Path {
        match self {
            Self::Workspace { root, .. } | Self::Package { root, .. } | Self::Standalone { root } => root,
        }
    }

    /// 当前模式下的包清单；standalone 返回 `None`。
    pub fn package_manifest(&self) -> Option<&PackageManifest> {
        match self {
            Self::Workspace { manifest, .. } => manifest.as_ref(),
            Self::Package { manifest, .. } => Some(manifest),
            Self::Standalone { .. } => None,
        }
    }

    /// 要求存在包清单，否则返回错误。
    pub fn expect_package_manifest(&self) -> Result<&PackageManifest> {
        self.package_manifest().ok_or_else(|| PackageManagerError::message("当前目录不是包（缺少包清单）"))
    }
}
