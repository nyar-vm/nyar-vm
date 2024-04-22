use nyar_workspace::{resolve_marker_root, sanitize_bucket_name};
use tempfile::tempdir;

#[test]
fn sanitize_replaces_dash_and_space() {
    assert_eq!(sanitize_bucket_name("wasm32-unknown browser"), "wasm32_unknown_browser");
}

#[test]
fn resolve_marker_finds_ancestor() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("legions.von"), "members: []\n").unwrap();
    let nested = root.join("packages").join("app");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(resolve_marker_root(&nested, "legions.von").as_deref(), Some(root));
}
