//! Nyar 包注册表门面。
//!
//! 渠道合同、第三方适配器与制品解包由 Boost 的 `package-registry` 提供；本 crate 只保留
//! 产品侧自有内容：
//!
//! - Valhalla 渠道（自有注册表与 Ed25519 发布者密钥）。
//! - 官方 CLI 凭据存储的发现与写回。凭证是宿主策略，因此不进入 Boost。
#![deny(missing_docs)]

pub mod credentials;
mod publisher_key;
mod valhalla;

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

pub use package_registry::*;

pub use credentials::{DiscoveredCredential, discover_token, sync_token_to_official_store, uses_external_credential_store};
pub use publisher_key::{PublisherKey, canonicalize_package_name, normalize_fingerprint};
pub use valhalla::ValhallaRegistry;

/// 注册表凭据的发现与写回（宿主策略）。
///
/// Boost 的 [`Registry`] 合同只拥有渠道请求，不认识任何官方 CLI 存储；需要按适配器 id
/// 读写 `.npmrc`、Deno 配置、NuGet.Config 等文件的宿主通过本扩展 trait 获得该能力。
pub trait RegistryCredentials {
    /// 该注册表是否只使用外部 CLI 凭据存储（不写入 `auth.von`）。
    fn uses_external_credentials(&self) -> bool;

    /// 从官方 CLI 配置发现令牌。
    fn discover_credential(&self, project_dir: Option<&Path>) -> Option<DiscoveredCredential>;

    /// 把已验证令牌写回对应的官方 CLI 存储。
    fn sync_credential(&self, token: &str) -> Result<Option<PathBuf>, RegistryError>;
}

impl<T: Registry + ?Sized> RegistryCredentials for T {
    fn uses_external_credentials(&self) -> bool {
        uses_external_credential_store(self.name())
    }

    fn discover_credential(&self, project_dir: Option<&Path>) -> Option<DiscoveredCredential> {
        discover_token(self.name(), Some(self.endpoint()), project_dir)
    }

    fn sync_credential(&self, token: &str) -> Result<Option<PathBuf>, RegistryError> {
        sync_token_to_official_store(self.name(), Some(self.endpoint()), token)
    }
}

/// 构建产品内置的注册表适配器集合：第三方渠道由 Boost 提供，Valhalla 由本 crate 追加。
pub fn default_registries() -> Result<HashMap<String, Arc<dyn Registry>>, RegistryError> {
    registries_with_endpoints(&BTreeMap::new())
}

/// 按 endpoint 覆盖构建产品内置适配器集合。
pub fn registries_with_endpoints(endpoints: &BTreeMap<String, String>) -> Result<HashMap<String, Arc<dyn Registry>>, RegistryError> {
    let mut registries = package_registry::registries_with_endpoints(endpoints)?;
    let endpoint = endpoints.get("valhalla").map(String::as_str).unwrap_or(ValhallaRegistry::DEFAULT_ENDPOINT);
    registries.insert("valhalla".to_string(), Arc::new(ValhallaRegistry::new(endpoint)?));
    Ok(registries)
}
