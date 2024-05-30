//! Credential *concepts* and delegation to package-registry adapters.
//!
//! This crate must not read or write ecosystem-specific stores (`.npmrc`, Deno config,
//! NuGet.Config, Maven settings, Anaconda nucleus, …). Concrete I/O lives in
//! [`nyar_package_registry`] (see `discover_token` / `sync_token_to_official_store`).

use std::path::{Path, PathBuf};

use crate::Result;

pub use nyar_package_registry::DiscoveredCredential;

/// Whether `registry` uses third-party CLI credential stores (not persisted in `auth.von`).
#[inline]
pub fn is_external_registry(registry: &str) -> bool {
    nyar_package_registry::uses_external_credential_store(registry)
}

/// Discover a token by delegating to the registry credential facade.
#[inline]
pub fn discover_token(registry: &str, endpoint: Option<&str>, project_dir: Option<&Path>) -> Option<DiscoveredCredential> {
    nyar_package_registry::discover_token(registry, endpoint, project_dir)
}

/// Sync a verified token into the official CLI store via the registry facade.
#[inline]
pub fn sync_token_to_official_store(registry: &str, endpoint: Option<&str>, token: &str) -> Result<Option<PathBuf>> {
    Ok(nyar_package_registry::sync_token_to_official_store(registry, endpoint, token)?)
}

/// 凭据发现与同步的概念层接口（注册表 id + 可选 endpoint）。
///
/// 若已持有 [`nyar_package_registry::Registry`] 适配器，优先调用其
/// [`Registry::discover_credential`]；本 trait 供字符串键编排使用。
pub trait TokenDiscovery {
    /// 注册表逻辑名称（如 `npm`、`valhalla`）。
    fn registry_id(&self) -> &str;

    /// 注册表 API 根地址；默认由适配器内置常量决定。
    fn registry_endpoint(&self) -> Option<&str> {
        None
    }

    /// 按注册表 id 发现可用 token，可结合 `project_dir` 读取官方 CLI 配置。
    fn discover_credential(&self, project_dir: Option<&Path>) -> Option<DiscoveredCredential> {
        discover_token(self.registry_id(), self.registry_endpoint(), project_dir)
    }

    /// 将已验证 token 写回该生态的官方 CLI 凭据存储。
    fn sync_credential(&self, token: &str) -> Result<Option<PathBuf>> {
        sync_token_to_official_store(self.registry_id(), self.registry_endpoint(), token)
    }

    /// 该注册表是否仅使用外部 CLI 凭据存储（不写入 `auth.von`）。
    fn uses_external_store(&self) -> bool {
        is_external_registry(self.registry_id())
    }
}

/// 由注册表 id 与 endpoint 组成的轻量凭据提供者，供 auth 编排层使用。
#[derive(Debug, Clone, Copy)]
pub struct RegistryCredentialId<'a> {
    /// 注册表逻辑名称。
    pub id: &'a str,
    /// 覆盖默认 endpoint 时的 API 根地址。
    pub endpoint: Option<&'a str>,
}

impl<'a> TokenDiscovery for RegistryCredentialId<'a> {
    fn registry_id(&self) -> &str {
        self.id
    }

    fn registry_endpoint(&self) -> Option<&str> {
        self.endpoint
    }
}
