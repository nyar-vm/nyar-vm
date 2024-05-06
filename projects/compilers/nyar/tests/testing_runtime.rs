use nyar::testing::runtime::{
    RuntimeFixtureResult, RuntimeFixtureSpec, load_runtime_fixture_spec, resolve_runtime_fixture_targets, verify_runtime_fixture_case,
};
use std::{cell::RefCell, collections::BTreeMap, fs};

#[test]
fn resolves_targets_from_sidecar_before_defaults() {
    let temp_dir = tempfile::tempdir().unwrap();
    let fixture_path = temp_dir.path().join("sample.demo");
    fs::write(&fixture_path, "fixture").unwrap();
    let sidecar = RuntimeFixtureSpec { targets: vec!["node".to_string(), "wasi".to_string()], expect: BTreeMap::new() };
    let yaml = serde_yaml::to_string(&sidecar).unwrap();
    fs::write(temp_dir.path().join("sample.demo.yaml"), yaml).unwrap();

    let targets = resolve_runtime_fixture_targets(&fixture_path, &["clr", "jvm"]);

    assert_eq!(targets, vec!["node".to_string(), "wasi".to_string()]);
}

#[test]
fn verifies_runtime_fixture_case_with_default_targets() {
    let temp_dir = tempfile::tempdir().unwrap();
    let fixture_path = temp_dir.path().join("sample.demo");
    fs::write(&fixture_path, "fixture").unwrap();
    let observed = RefCell::new(Vec::new());

    verify_runtime_fixture_case(&fixture_path, &["clr", "node"], false, |target| {
        observed.borrow_mut().push(target.to_string());
        RuntimeFixtureResult {
            success: true,
            stdout: vec![format!("hello-{target}")],
            stderr: Vec::new(),
            allow_stderr: false,
            errors: Vec::new(),
            result: Some(0),
        }
    });

    assert_eq!(observed.into_inner(), vec!["clr".to_string(), "node".to_string()]);
    let saved = load_runtime_fixture_spec(&fixture_path).unwrap();
    assert_eq!(saved.targets, vec!["clr".to_string(), "node".to_string()]);
    assert_eq!(saved.expect["clr"].stdout, vec!["hello-clr".to_string()]);
    assert_eq!(saved.expect["node"].stdout, vec!["hello-node".to_string()]);
}
