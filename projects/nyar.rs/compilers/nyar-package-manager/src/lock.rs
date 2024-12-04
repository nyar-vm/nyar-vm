use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use nyar_package_registry::Package;
use oak_von::from_str;
use serde::{Deserialize, Serialize};

use crate::{ProjectLayout, Result};

/// 锁文件中的单个已解析依赖条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LockEntry {
    /// 包名。
    pub name: String,
    /// 锁定版本。
    pub version: String,
    /// 来源注册表名称。
    #[serde(default)]
    pub registry: String,
    /// 注册表返回的解析 URL 或 tarball 地址。
    #[serde(default)]
    pub resolved: String,
    /// 子资源完整性（SRI）摘要。
    #[serde(default)]
    pub integrity: String,
    /// SPDX 或注册表声明的许可证。
    #[serde(default)]
    pub license: String,
    /// 是否为开发依赖。
    #[serde(default)]
    pub is_dev: bool,
    /// 是否来自 workspace 成员而非远程注册表。
    #[serde(default)]
    pub is_workspace: bool,
    /// 相对 `vendors/` 的安装路径。
    #[serde(default)]
    pub install_path: String,
    /// 直接依赖包名列表（与注册表元数据一致）。
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// 项目锁文件；磁盘文件名由 [`ProjectLayout::lockfile`] 指定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LockFile {
    /// 锁文件格式版本。
    #[serde(default = "default_lock_version")]
    pub version: String,
    /// `name@version` 到条目的映射。
    #[serde(default)]
    pub packages: BTreeMap<String, LockEntry>,
    #[serde(skip)]
    path: PathBuf,
}

fn default_lock_version() -> String {
    "1".to_string()
}

impl LockFile {
    /// 在 `directory` 下创建空锁文件句柄（尚未写入磁盘）。
    pub fn new(directory: impl AsRef<Path>, layout: ProjectLayout) -> Self {
        Self { version: default_lock_version(), packages: BTreeMap::new(), path: layout.lockfile_path(directory) }
    }

    /// 锁文件绝对路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 锁文件是否已存在于磁盘。
    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    /// 从 `directory` 加载锁文件；不存在时返回空锁。
    pub fn load(directory: impl AsRef<Path>, layout: ProjectLayout) -> Result<Self> {
        let path = layout.lockfile_path(&directory);
        if !path.is_file() {
            return Ok(Self::new(directory, layout));
        }
        let source = std::fs::read_to_string(&path)?;
        let mut lock: LockFile = from_str(&source)?;
        lock.path = path;
        Ok(lock)
    }

    /// 将当前锁状态序列化并写入磁盘。
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = crate::von_format::render_lock_file(&self.version, &self.packages);
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    /// 生成锁文件使用的 `name@version` 键。
    pub fn package_key(name: &str, version: &str) -> String {
        format!("{name}@{version}")
    }

    /// 根据注册表返回的 [`Package`] 元数据插入或覆盖一条锁记录。
    pub fn add_package(&mut self, package: &Package, registry_name: &str, registry_endpoint: &str) {
        let key = Self::package_key(&package.name, &package.version);
        self.packages.insert(
            key,
            LockEntry {
                name: package.name.clone(),
                version: package.version.clone(),
                registry: registry_name.to_string(),
                resolved: package.dist_tarball.clone().unwrap_or_else(|| format!("{registry_endpoint}/{}", package.name)),
                integrity: package.dist_integrity.clone().unwrap_or_default(),
                license: package.license.clone(),
                is_dev: false,
                is_workspace: false,
                // Relative to `vendors/`; matches `PackageManager::package_install_path`.
                install_path: format!("{registry_name}/{}@{}", package.name, package.version),
                dependencies: package.dependencies.clone(),
            },
        );
    }

    /// 按包名移除所有版本条目。
    pub fn remove_package(&mut self, name: &str) {
        self.packages.retain(|_, entry| entry.name != name);
    }

    /// 对比期望的 `(name, version)` 列表，返回缺失锁定的描述信息。
    pub fn detect_drift(&self, expected: &[(String, String)]) -> Vec<String> {
        let mut drift = Vec::new();
        for (name, version) in expected {
            let key = Self::package_key(name, version);
            if !self.packages.contains_key(&key) {
                drift.push(format!("missing locked package {key}"));
            }
        }
        drift
    }
}
