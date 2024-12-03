use nyar::testing::legend_fixtures::{load_legend_fixture_manifest, resolve_legend_fixture_paths};
use std::path::PathBuf;

#[test]
fn loads_and_resolves_manifest_entries() {
    let temp_dir = tempfile::tempdir().unwrap();
    let fixtures_dir = temp_dir.path().join("fixtures");
    std::fs::create_dir_all(fixtures_dir.join("lua")).unwrap();
    std::fs::write(fixtures_dir.join("lua").join("hello.lua"), "print('hello')").unwrap();
    let manifest_path = fixtures_dir.join("manifest.yaml");
    std::fs::write(
        &manifest_path,
        r#"fixtures:
  - language: lua
    path: lua/hello.lua
    targets: [legend-vm]
"#,
    )
    .unwrap();

    let manifest = load_legend_fixture_manifest(&manifest_path);
    assert_eq!(manifest.fixtures.len(), 1);
    let paths = resolve_legend_fixture_paths(&manifest_path, &manifest);
    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("lua\\hello.lua") || paths[0].ends_with("lua/hello.lua"));
}

#[test]
fn loads_workspace_legend_manifest() {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../legend/fixtures/manifest.yaml");
    if !manifest_path.exists() {
        eprintln!("skip loads_workspace_legend_manifest: missing {}", manifest_path.display());
        return;
    }
    let manifest = load_legend_fixture_manifest(&manifest_path);
    assert!(!manifest.fixtures.is_empty(), "legend fixture manifest should not be empty");
    let paths = resolve_legend_fixture_paths(&manifest_path, &manifest);
    assert_eq!(paths.len(), manifest.fixtures.len());
    for path in paths {
        assert!(path.exists(), "legend fixture path must exist: {}", path.display());
    }
}
