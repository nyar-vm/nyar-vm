use nyar_package_manager::*;
use tempfile::TempDir;

#[test]
fn wasm_maps_to_node_for_npm() {
    let dir = TempDir::new().expect("temp");
    let artifact = dir.path().join("dist").join("wasm32-node-unknown-wasm");
    std::fs::create_dir_all(&artifact).expect("mkdir");
    let resolved = resolve_publish_artifact(dir.path(), "wasm", "npm").expect("resolve");
    assert_eq!(resolved.canonical_label, "wasm32-node-unknown-wasm");
}

#[test]
fn missing_dist_errors_with_hint() {
    let dir = TempDir::new().expect("temp");
    let error = resolve_publish_artifact(dir.path(), "node", "npm").expect_err("missing dist");
    assert!(error.to_string().contains("build --target"));
}
