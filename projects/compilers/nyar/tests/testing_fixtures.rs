use nyar::testing::fixtures::{
    assert_or_regenerate_text_sidecar, assert_or_regenerate_yaml_sidecar, collect_fixture_cases_with_extensions, load_optional_text_sidecar,
    load_optional_yaml_sidecar, text_sidecar_path,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DemoFixture {
    value: String,
}

#[test]
fn sidecar_is_created_on_first_run() {
    let temp_dir = tempfile::tempdir().unwrap();
    let fixture_path = temp_dir.path().join("sample.demo");
    fs::write(&fixture_path, "fixture").unwrap();

    let observed = DemoFixture { value: "first".to_string() };
    assert_or_regenerate_yaml_sidecar(&fixture_path, &observed, false);

    let saved = load_optional_yaml_sidecar::<DemoFixture>(&fixture_path).unwrap();
    assert_eq!(saved, observed);
    assert!(Path::new(&format!("{}.yaml", fixture_path.display())).exists());
}

#[test]
fn text_sidecar_is_created_on_first_run() {
    let temp_dir = tempfile::tempdir().unwrap();
    let fixture_path = temp_dir.path().join("sample.demo");
    fs::write(&fixture_path, "fixture").unwrap();

    assert_or_regenerate_text_sidecar(&fixture_path, "lex", "Token { kind: Eof }\n", false);

    let saved = load_optional_text_sidecar(&text_sidecar_path(&fixture_path, "lex")).unwrap();
    assert_eq!(saved, "Token { kind: Eof }\n");
}

#[test]
fn text_sidecar_can_be_regenerated() {
    let temp_dir = tempfile::tempdir().unwrap();
    let fixture_path = temp_dir.path().join("sample.demo");
    fs::write(&fixture_path, "fixture").unwrap();

    assert_or_regenerate_text_sidecar(&fixture_path, "lex", "first\n", false);
    assert_or_regenerate_text_sidecar(&fixture_path, "lex", "second\n", true);

    let saved = load_optional_text_sidecar(&text_sidecar_path(&fixture_path, "lex")).unwrap();
    assert_eq!(saved, "second\n");
}

#[test]
fn collection_filters_requested_extensions() {
    let temp_dir = tempfile::tempdir().unwrap();
    fs::write(temp_dir.path().join("one.demo"), "").unwrap();
    fs::write(temp_dir.path().join("two.txt"), "").unwrap();
    fs::create_dir_all(temp_dir.path().join("nested")).unwrap();
    fs::write(temp_dir.path().join("nested").join("three.DEMO"), "").unwrap();

    let cases = collect_fixture_cases_with_extensions(temp_dir.path(), &["demo"]);

    assert_eq!(cases.len(), 2);
    assert!(
        cases.iter().all(|path| { path.extension().and_then(|value| value.to_str()).is_some_and(|value| value.eq_ignore_ascii_case("demo")) })
    );
}
