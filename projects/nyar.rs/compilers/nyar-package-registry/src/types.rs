use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Package metadata returned by a registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Package {
    /// Fully qualified package name.
    pub name: String,
    /// Resolved semantic version string.
    pub version: String,
    /// Short package description.
    #[serde(default)]
    pub description: String,
    /// Project homepage URL.
    #[serde(default)]
    pub homepage: String,
    /// Primary author or maintainer label.
    #[serde(default)]
    pub author: String,
    /// SPDX or registry-specific license identifier.
    #[serde(default)]
    pub license: String,
    /// Direct dependency names (legacy flat list).
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Dependency name to pinned version map.
    #[serde(default)]
    pub dependency_versions: BTreeMap<String, String>,
    /// Target triple or platform to compatible version constraints.
    #[serde(default)]
    pub target_conditions: BTreeMap<String, Vec<String>>,
    /// Remote tarball URL when the registry exposes one.
    pub dist_tarball: Option<String>,
    /// Subresource integrity hash for `dist_tarball`, when present.
    pub dist_integrity: Option<String>,
    /// Whether the package is published as an SDK module (JSR-style).
    #[serde(default)]
    pub is_sdk_package: bool,
    /// SDK module export name when `is_sdk_package` is true.
    pub sdk_module_name: Option<String>,
    /// Optional peer dependency constraints.
    pub peer_dependencies: Option<BTreeMap<String, String>>,
}

/// Version bump kind for publish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionBump {
    /// Increment patch component.
    Patch,
    /// Increment minor component.
    Minor,
    /// Increment major component.
    Major,
}

impl VersionBump {
    /// Parse a bump keyword (`patch`, `minor`, `major`).
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "patch" => Some(Self::Patch),
            "minor" => Some(Self::Minor),
            "major" => Some(Self::Major),
            _ => None,
        }
    }
}

/// Options controlling package publish.
#[derive(Debug, Clone)]
pub struct PublishOptions {
    /// Package name being published.
    pub package_name: String,
    /// Version string to publish.
    pub version: String,
    /// Publish-time description override.
    pub description: String,
    /// Optional homepage override.
    pub homepage: Option<String>,
    /// Optional license override.
    pub license: Option<String>,
    /// Local path to the packaged artifact directory or tarball root.
    pub package_path: String,
    /// Logical registry adapter name (for example `npm` or `valhalla`).
    pub registry_name: String,
    /// Bearer token or publisher key material.
    pub auth_token: Option<String>,
    /// Distribution tag (npm-style) when supported.
    pub tag: Option<String>,
    /// Access level (`public` / `restricted`) when supported.
    pub access: Option<String>,
    /// Automatic semver bump to apply before upload.
    pub bump: Option<VersionBump>,
    /// Whether to create a git tag after a successful publish.
    pub create_git_tag: bool,
    /// Prefix prepended to generated git tags.
    pub git_tag_prefix: Option<String>,
    /// Skip clean working-tree checks before publish.
    pub skip_git_check: bool,
    /// Run the package `prepublish` script when present.
    pub run_pre_publish_script: bool,
    /// Validate and package without uploading.
    pub dry_run: bool,
    /// When set, pack from this directory (registry artifact layout) instead of the project root.
    pub artifact_dir: Option<String>,
    /// Glob patterns from package-manifest `files` merged into the publish tarball.
    pub include_files: Vec<String>,
    /// JSR tarballs use a flat root layout (no `package/` prefix).
    pub flat_layout: bool,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self {
            package_name: String::new(),
            version: String::new(),
            description: String::new(),
            homepage: None,
            license: None,
            package_path: String::new(),
            registry_name: String::new(),
            auth_token: None,
            tag: Some("latest".to_string()),
            access: Some("public".to_string()),
            bump: None,
            create_git_tag: true,
            git_tag_prefix: Some("v".to_string()),
            skip_git_check: false,
            run_pre_publish_script: true,
            dry_run: false,
            artifact_dir: None,
            include_files: Vec::new(),
            flat_layout: false,
        }
    }
}

/// Result of a publish attempt.
#[derive(Debug, Clone, Default)]
pub struct PublishResult {
    /// Whether the registry accepted the upload (or dry-run succeeded).
    pub success: bool,
    /// Published package name.
    pub package_name: String,
    /// Published version string.
    pub version: String,
    /// Human-readable status or error summary.
    pub message: String,
    /// Canonical registry URL for the new release, when available.
    pub published_url: Option<String>,
    /// True when the operation was a dry run.
    pub dry_run: bool,
    /// SHA-256 digest of the uploaded artifact.
    pub sha256: Option<String>,
    /// Byte size of the uploaded artifact.
    pub size: Option<usize>,
    /// Number of files included in the upload bundle.
    pub file_count: Option<usize>,
    /// True when the registry expects users to publish via an official CLI.
    pub official_tool_required: bool,
}

impl PublishResult {
    /// Build a structured result for registries that require official tooling.
    pub fn official_tool(
        package_name: impl Into<String>,
        version: impl Into<String>,
        message: impl Into<String>,
        published_url: Option<String>,
        size: usize,
    ) -> Self {
        Self {
            success: false,
            package_name: package_name.into(),
            version: version.into(),
            message: message.into(),
            published_url,
            dry_run: false,
            sha256: None,
            size: Some(size),
            file_count: None,
            official_tool_required: true,
        }
    }
}

/// Token verification outcome.
#[derive(Debug, Clone)]
pub struct TokenVerifyResult {
    /// Whether the token is valid for the registry.
    pub valid: bool,
    /// Authenticated username or publisher id when valid.
    pub username: Option<String>,
    /// Error detail when `valid` is false.
    pub error_message: Option<String>,
}

impl TokenVerifyResult {
    /// Valid token with an associated username.
    pub fn success(username: impl Into<String>) -> Self {
        Self { valid: true, username: Some(username.into()), error_message: None }
    }

    /// Invalid or rejected token.
    pub fn failure(message: impl Into<String>) -> Self {
        Self { valid: false, username: None, error_message: Some(message.into()) }
    }
}

/// Retry configuration for registry HTTP calls.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts after the first request.
    pub max_retries: u32,
    /// Initial backoff delay in milliseconds.
    pub initial_delay_ms: u64,
    /// Multiplier applied to the delay after each failed attempt.
    pub backoff_multiplier: f64,
    /// Upper bound on backoff delay in milliseconds.
    pub max_delay_ms: u64,
    /// TCP connect timeout in seconds.
    pub connect_timeout_secs: u64,
    /// Per-request timeout in seconds (non-upload).
    pub request_timeout_secs: u64,
    /// Upload request timeout in seconds.
    pub upload_timeout_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 500,
            backoff_multiplier: 2.0,
            max_delay_ms: 10_000,
            connect_timeout_secs: 30,
            request_timeout_secs: 60,
            upload_timeout_secs: 600,
        }
    }
}

impl RetryConfig {
    /// Policy tuned for npm-compatible registries.
    pub fn npm() -> Self {
        Self { max_retries: 5, initial_delay_ms: 500, backoff_multiplier: 2.0, max_delay_ms: 15_000, ..Self::default() }
    }
}
