use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use nyar_package_registry::Package;
use serde::{Deserialize, Serialize};

use crate::{PackageManagerError, Result};

const OSV_ENDPOINT: &str = "https://api.osv.dev/v1/query";

/// Vulnerability report for a locked package.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VulnerabilityReport {
    /// 受影响的包名。
    pub package_name: String,
    /// 受影响的版本。
    pub version: String,
    /// OSV / CVE 等漏洞编号。
    pub vulnerability_id: Option<String>,
    /// 漏洞摘要标题。
    pub title: String,
    /// 严重级别（来自 OSV `database_specific` 或 `unknown`）。
    pub severity: String,
    /// 建议升级到的修复版本。
    pub fixed_version: Option<String>,
    /// 参考链接（公告、 advisory 等）。
    pub url: Option<String>,
}

/// License audit entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LicenseInfo {
    /// 被审计的包名。
    pub package_name: String,
    /// 被审计的版本。
    pub version: String,
    /// 声明的 SPDX 许可证字符串。
    pub license: String,
    /// 是否在允许列表（MIT、Apache-2.0 等）内。
    pub is_compatible: bool,
    /// 是否属于 copyleft / 商业限制许可证。
    pub is_restricted: bool,
}

/// Combined security audit result.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityAuditResult {
    /// OSV 查询得到的漏洞条目。
    pub vulnerabilities: Vec<VulnerabilityReport>,
    /// 许可证合规检查结果。
    pub licenses: Vec<LicenseInfo>,
}

impl SecurityAuditResult {
    /// 是否发现至少一条漏洞报告。
    pub fn has_vulnerabilities(&self) -> bool {
        !self.vulnerabilities.is_empty()
    }

    /// 是否存在不兼容或受限许可证。
    pub fn has_license_issues(&self) -> bool {
        self.licenses.iter().any(|license| !license.is_compatible || license.is_restricted)
    }

    /// 合并另一份审计结果（用于批量依赖扫描）。
    pub fn merge(&mut self, other: SecurityAuditResult) {
        self.vulnerabilities.extend(other.vulnerabilities);
        self.licenses.extend(other.licenses);
    }

    /// 按 `name@version` 汇总建议升级到的最高修复版本。
    pub fn fix_suggestions(&self) -> BTreeMap<String, String> {
        let mut suggestions = BTreeMap::new();
        for report in &self.vulnerabilities {
            let Some(fixed) = &report.fixed_version
            else {
                continue;
            };
            let key = format!("{}@{}", report.package_name, report.version);
            suggestions
                .entry(key)
                .and_modify(|current: &mut String| {
                    if fixed.as_str() > current.as_str() {
                        *current = fixed.clone();
                    }
                })
                .or_insert_with(|| fixed.clone());
        }
        suggestions
    }
}

/// OSV + license policy engine.
pub struct SecurityAudit {
    cache_path: PathBuf,
    cache: BTreeMap<String, Vec<VulnerabilityReport>>,
    skip_network: bool,
}

impl SecurityAudit {
    /// Open using the product layout cache directory.
    pub fn open_with_layout(layout: crate::ProjectLayout) -> Result<Self> {
        let cache_dir = layout.cache_dir();
        std::fs::create_dir_all(&cache_dir)?;
        Self::with_cache_path(cache_dir.join("vulnerability-cache.json"))
    }

    /// Open with neutral layout paths (tests / tools without a product CLI).
    pub fn open_default() -> Result<Self> {
        Self::open_with_layout(crate::ProjectLayout::neutral())
    }

    /// 使用自定义 OSV 查询结果缓存文件路径。
    pub fn with_cache_path(cache_path: impl AsRef<Path>) -> Result<Self> {
        let cache_path = cache_path.as_ref().to_path_buf();
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let cache = load_cache(&cache_path);
        Ok(Self { cache_path, cache, skip_network: false })
    }

    /// Disable outbound OSV calls (unit tests / offline).
    pub fn offline(mut self) -> Self {
        self.skip_network = true;
        self
    }

    /// 对单个包执行 OSV 漏洞扫描与许可证策略检查。
    pub fn audit_package(&mut self, package: &Package, registry: &str) -> Result<SecurityAuditResult> {
        Ok(SecurityAuditResult { vulnerabilities: self.scan_vulnerabilities(package, registry)?, licenses: vec![check_license(package)] })
    }

    /// 批量审计多个 `(Package, registry)` 并合并结果。
    pub fn audit_dependencies(&mut self, packages: &[(Package, String)]) -> Result<SecurityAuditResult> {
        let mut result = SecurityAuditResult::default();
        for (package, registry) in packages {
            result.merge(self.audit_package(package, registry)?);
        }
        Ok(result)
    }

    /// 判断许可证是否在项目允许列表内。
    pub fn is_license_compatible(license: &str) -> bool {
        is_license_compatible(license)
    }

    /// 判断许可证是否属于 GPL/AGPL 等受限类别。
    pub fn is_license_restricted(license: &str) -> bool {
        is_license_restricted(license)
    }

    fn scan_vulnerabilities(&mut self, package: &Package, registry: &str) -> Result<Vec<VulnerabilityReport>> {
        let key = format!("{}@{}", package.name, package.version);
        if let Some(cached) = self.cache.get(&key) {
            return Ok(cached.clone());
        }

        let Some(ecosystem) = ecosystem_for_registry(registry)
        else {
            return Ok(Vec::new());
        };

        if self.skip_network {
            return Ok(Vec::new());
        }

        let reports = match query_osv(&package.name, &package.version, ecosystem) {
            Ok(reports) => reports,
            Err(error) => {
                eprintln!("warning: OSV query failed for {key}: {error}");
                Vec::new()
            }
        };
        self.cache.insert(key, reports.clone());
        let _ = save_cache(&self.cache_path, &self.cache);
        Ok(reports)
    }
}

fn ecosystem_for_registry(registry: &str) -> Option<&'static str> {
    match registry.to_ascii_lowercase().as_str() {
        "npm" | "jsr" => Some("npm"),
        "nuget" => Some("NuGet"),
        "maven" => Some("Maven"),
        "pypi" | "conda" => Some("PyPI"),
        "crates" | "crates-io" => Some("crates.io"),
        "valhalla" | "local" | "mock" | "workspace" => None,
        _ => None,
    }
}

/// 计算包的许可证兼容 / 受限信息（供集成测试）。
#[doc(hidden)]
pub fn check_license(package: &Package) -> LicenseInfo {
    let license = if package.license.trim().is_empty() { "unknown".to_string() } else { package.license.clone() };
    let restricted = is_license_restricted(&license);
    let compatible = is_license_compatible(&license) && !restricted;
    LicenseInfo {
        package_name: package.name.clone(),
        version: package.version.clone(),
        license,
        is_compatible: compatible,
        is_restricted: restricted,
    }
}

fn is_license_compatible(license: &str) -> bool {
    if license.eq_ignore_ascii_case("unknown") {
        return false;
    }
    COMPATIBLE.iter().any(|item| license.eq_ignore_ascii_case(item))
}

fn is_license_restricted(license: &str) -> bool {
    RESTRICTED.iter().any(|item| license.eq_ignore_ascii_case(item))
}

const COMPATIBLE: &[&str] = &["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "0BSD", "ISC", "Unlicense", "CC0-1.0", "WTFPL", "Zlib"];

const RESTRICTED: &[&str] =
    &["GPL-2.0-only", "GPL-2.0-or-later", "GPL-3.0-only", "GPL-3.0-or-later", "AGPL-3.0-only", "AGPL-3.0-or-later", "SSPL-1.0", "BUSL-1.1"];

fn query_osv(name: &str, version: &str, ecosystem: &str) -> Result<Vec<VulnerabilityReport>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("nyar-package-manager/0.1")
        .build()
        .map_err(|error| PackageManagerError::message(error.to_string()))?;

    let payload = serde_json::json!({
        "package": { "name": name, "ecosystem": ecosystem },
        "version": version
    });

    let response = client.post(OSV_ENDPOINT).json(&payload).send().map_err(|error| PackageManagerError::message(error.to_string()))?;
    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let body: OsvResponse = response.json().map_err(|error| PackageManagerError::message(error.to_string()))?;
    let mut reports = Vec::new();
    for vuln in body.vulns.unwrap_or_default() {
        let vulnerability_id = vuln.id.or_else(|| vuln.aliases.unwrap_or_default().into_iter().find(|alias| alias.starts_with("CVE-")));
        let severity = vuln
            .database_specific
            .as_ref()
            .and_then(|value| value.get("severity"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string();
        let fixed_version = vuln.affected.as_ref().and_then(|affected| extract_fixed_version(affected));
        let url = vuln
            .references
            .as_ref()
            .and_then(|refs| refs.first())
            .and_then(|reference| reference.get("url"))
            .and_then(|value| value.as_str())
            .map(str::to_string);

        reports.push(VulnerabilityReport {
            package_name: name.to_string(),
            version: version.to_string(),
            vulnerability_id,
            title: vuln.summary.unwrap_or_default(),
            severity,
            fixed_version,
            url,
        });
    }
    Ok(reports)
}

fn extract_fixed_version(affected: &[serde_json::Value]) -> Option<String> {
    for item in affected {
        let ranges = item.get("ranges")?.as_array()?;
        for range in ranges {
            let events = range.get("events")?.as_array()?;
            for event in events {
                if let Some(fixed) = event.get("fixed").and_then(|value| value.as_str()) {
                    return Some(fixed.to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct OsvResponse {
    vulns: Option<Vec<OsvVuln>>,
}

#[derive(Debug, Deserialize)]
struct OsvVuln {
    id: Option<String>,
    summary: Option<String>,
    aliases: Option<Vec<String>>,
    database_specific: Option<serde_json::Value>,
    affected: Option<Vec<serde_json::Value>>,
    references: Option<Vec<serde_json::Value>>,
}

fn load_cache(path: &Path) -> BTreeMap<String, Vec<VulnerabilityReport>> {
    if !path.is_file() {
        return BTreeMap::new();
    }
    std::fs::read_to_string(path).ok().and_then(|source| serde_json::from_str(&source).ok()).unwrap_or_default()
}

fn save_cache(path: &Path, cache: &BTreeMap<String, Vec<VulnerabilityReport>>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cache).map_err(|error| PackageManagerError::message(error.to_string()))?;
    std::fs::write(path, json)?;
    Ok(())
}
