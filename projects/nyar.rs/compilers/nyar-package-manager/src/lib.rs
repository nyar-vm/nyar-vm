//! Nyar 包管理器门面。
//!
//! 清单、lock、求解、缓存、打包发布与编排都由 Boost 的 `package.*` 提供。本 crate 只保留
//! 产品身份：工程布局常量、自有渠道（Valhalla）、官方 CLI 凭据策略与代理取值。
#![deny(missing_docs)]

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
};

/// 产品布局：清单 / lockfile 文件名与用户数据目录。
pub use package_resolve::ProjectLayout;

pub use package_cache::PackageCache;
pub use package_manager::*;

/// 注册表值类型与适配器合同由产品门面继续导出。
pub use nyar_package_registry::{
    MockRegistry, Package, PublishOptions, PublishResult, PublisherKey, Registry, RegistryError, TokenVerifyResult, VersionBump,
};

/// 该注册表是否只使用第三方 CLI 凭据存储。
pub fn is_external_registry(registry: &str) -> bool {
    nyar_package_registry::credentials::uses_external_credential_store(registry)
}

/// 产品工程布局：唯一允许拼写产品文件名的位置。
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

/// 与本产品错误类型同名的别名，供旧调用点使用。
pub type PackageManagerError = ManagerError;

/// 产品凭据策略：读官方 CLI 配置，但不让包机制直接接触用户秘密。
#[derive(Debug, Default, Clone, Copy)]
pub struct OfficialCliCredentials;

impl CredentialProvider for OfficialCliCredentials {
    fn uses_external_store(&self, registry: &str) -> bool {
        nyar_package_registry::credentials::uses_external_credential_store(registry)
    }

    fn discover(&self, registry: &str, endpoint: Option<&str>, project_dir: Option<&Path>) -> Option<DiscoveredCredential> {
        nyar_package_registry::credentials::discover_token(registry, endpoint, project_dir).map(|item| DiscoveredCredential {
            token: item.token,
            source: item.source,
        })
    }

    fn sync_to_official(&self, registry: &str, endpoint: Option<&str>, token: &str) -> Result<Option<PathBuf>> {
        Ok(nyar_package_registry::credentials::sync_token_to_official_store(registry, endpoint, token)?)
    }
}

/// 产品注册表工厂：第三方渠道 + 自有渠道，并按宿主策略应用代理。
pub fn product_registry_factory() -> RegistryFactory {
    Arc::new(|endpoints: &BTreeMap<String, String>| {
        let proxy = nyar_package_registry::ProxyConfig::from_env().ok().flatten();
        let mut registries = match &proxy {
            Some(proxy) => nyar_package_registry::registries_with_proxy(endpoints, proxy.clone())?,
            None => nyar_package_registry::registries_with_endpoints(endpoints)?,
        };
        let endpoint = endpoints.get("valhalla").map(String::as_str).unwrap_or(nyar_package_registry::ValhallaRegistry::DEFAULT_ENDPOINT);
        let valhalla = match proxy {
            Some(proxy) => nyar_package_registry::ValhallaRegistry::with_http(
                endpoint,
                nyar_package_registry::HttpClient::with_proxy(nyar_package_registry::RetryConfig::default(), proxy),
            ),
            None => nyar_package_registry::ValhallaRegistry::new(endpoint)?,
        };
        registries.insert("valhalla".to_string(), Arc::new(valhalla));
        Ok(registries)
    })
}

/// 产品默认服务：自有渠道工厂 + 官方 CLI 凭据策略。
pub fn product_services() -> ManagerServices {
    ManagerServices::new(product_registry_factory(), Arc::new(OfficialCliCredentials))
}

/// 按产品服务打开包管理器。
pub fn open(directory: impl AsRef<Path>) -> Result<PackageManager> {
    PackageManager::open_with_services(directory, PRODUCT_LAYOUT, product_services())
}

/// 按产品服务打开注册表 endpoint 配置。
pub fn open_registry_sources() -> Result<RegistrySourceManager> {
    RegistrySourceManager::open_with_factory(PRODUCT_LAYOUT.registry_sources_path(), product_registry_factory())
}

/// 供产品内部使用的注册表 endpoint 表类型。
pub type RegistryEndpoints = BTreeMap<String, String>;

/// 供产品内部复用的注册表表类型。
pub type RegistryMap = HashMap<String, Arc<dyn nyar_package_registry::Registry>>;
