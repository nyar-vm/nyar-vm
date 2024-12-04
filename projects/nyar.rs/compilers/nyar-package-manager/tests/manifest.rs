use nyar_package_manager::{ProjectLayout, *};

#[test]
fn save_roundtrip_dev_dependencies() {
    let manifest = PackageManifest::parse(
        "{\n    name: \"smoke.root\",\n    version: \"0.1.0\",\n    dev_dependencies: {\n        \"smoke.dev\": \"1.5.0\",\n    },\n}\n",
    )
    .expect("parse");
    let dir = tempfile::tempdir().expect("tempdir");
    manifest.save(dir.path(), ProjectLayout::neutral()).expect("save");
    let loaded = PackageManifest::load(dir.path(), ProjectLayout::neutral()).expect("load");
    assert_eq!(loaded.dev_dependencies.get("smoke.dev").and_then(|s| s.version_constraint()), Some("1.5.0"));
}
