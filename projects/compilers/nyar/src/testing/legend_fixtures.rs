use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use super::runtime::{RuntimeFixtureResult, RuntimeFixtureSpec, verify_runtime_fixture_spec};

/// A single legend fixture entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LegendFixtureEntry {
    /// Language identifier, such as `lua`, `tcl`, `bash`.
    pub language: String,
    /// Relative fixture file path from manifest directory.
    pub path: String,
    /// Optional target list for runtime checks.
    #[serde(default)]
    pub targets: Vec<String>,
}

/// Manifest for legend fixture files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LegendFixtureManifest {
    /// Declared fixture entries.
    #[serde(default)]
    pub fixtures: Vec<LegendFixtureEntry>,
}

/// Load a legend fixture manifest from YAML file.
pub fn load_legend_fixture_manifest(manifest_path: &Path) -> LegendFixtureManifest {
    let source = fs::read_to_string(manifest_path)
        .unwrap_or_else(|error| panic!("failed to read legend fixture manifest '{}': {}", manifest_path.display(), error));
    serde_yaml::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse legend fixture manifest '{}': {}", manifest_path.display(), error))
}

/// Resolve absolute paths for every fixture entry.
pub fn resolve_legend_fixture_paths(manifest_path: &Path, manifest: &LegendFixtureManifest) -> Vec<PathBuf> {
    let base = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    manifest.fixtures.iter().map(|entry| base.join(&entry.path)).collect()
}

/// Resolve runtime targets for a legend fixture entry.
pub fn resolve_legend_fixture_targets(entry: &LegendFixtureEntry, default_targets: &[&str]) -> Vec<String> {
    if !entry.targets.is_empty() { entry.targets.clone() } else { default_targets.iter().map(|target| (*target).to_string()).collect() }
}

/// Verify a legend fixture case using the shared runtime sidecar machinery.
pub fn verify_legend_fixture_case<F>(fixture_path: &Path, entry: &LegendFixtureEntry, regenerate: bool, mut observe: F)
where
    F: FnMut(&str) -> RuntimeFixtureResult,
{
    let targets = resolve_legend_fixture_targets(entry, &["legacy-vm"]);
    assert!(!targets.is_empty(), "legend fixture '{}' has no targets", fixture_path.display());
    let mut expect = BTreeMap::new();
    for target in &targets {
        expect.insert(target.clone(), observe(target));
    }
    let observed = RuntimeFixtureSpec { targets, expect };
    verify_runtime_fixture_spec(fixture_path, &observed, regenerate);
}
