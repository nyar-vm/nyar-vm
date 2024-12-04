use nyar_package_manager::*;

#[test]
fn resolves_unicode_scope_entry_to_mjs() {
    let dir = tempfile::tempdir().expect("temp");
    let artifact = dir.path().join("dist");
    std::fs::create_dir_all(&artifact).expect("mkdir");
    std::fs::write(artifact.join("demo_cli.mjs"), "export {}").expect("write mjs");
    std::fs::write(artifact.join("demo_cli.wasm"), b"\0asm").expect("write wasm");
    std::fs::write(
            artifact.join("run-contracts.txt"),
            "{\n    run_contracts: [\n        {\n            logical_entry: \"demo::cli\",\n            physical_entry: \"demo_cli.mjs\",\n            invocation: \"node\",\n            validate: \"node demo_cli.mjs\"\n        }\n    ]\n}\n",
        )
        .expect("write contract");

    let options = RegistryPackOptions {
        artifact_dir: artifact.clone(),
        package_root: dir.path().to_path_buf(),
        registry: "npm".to_string(),
        meta: PackMeta {
            name: "@scope/pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            license: Some("MIT".to_string()),
        },
        include_files: Vec::new(),
        flat_layout: false,
        layout: nyar_package_manager::ProjectLayout::neutral(),
    };
    let packed = pack_registry_artifact(&options).expect("pack");
    assert!(packed.file_count >= 3);

    let unpack_dir = dir.path().join("unpacked");
    std::fs::create_dir_all(&unpack_dir).expect("unpack dir");
    let count = unpack(&packed.tarball_data, &unpack_dir).expect("unpack");
    assert!(count >= 3);
    let package_json = std::fs::read_to_string(unpack_dir.join("package.json")).expect("package.json");
    assert!(package_json.contains("demo_cli.mjs"));
    assert!(package_json.contains("\"cli\""));
}

#[test]
fn packs_manifest_files_and_jsr_flat_layout() {
    let dir = tempfile::tempdir().expect("temp");
    let artifact = dir.path().join("dist").join("wasm32-node-unknown-wasm");
    std::fs::create_dir_all(artifact.join("docs")).expect("mkdir");
    std::fs::write(artifact.join("demo_cli.mjs"), "export {}").expect("write mjs");
    std::fs::write(artifact.join("demo_cli.wasm"), b"\0asm").expect("write wasm");
    std::fs::write(artifact.join("docs").join("extra.md"), "# extra").expect("write extra");
    std::fs::write(dir.path().join("NOTICE"), "notice").expect("write notice");
    std::fs::write(
            artifact.join("run-contracts.txt"),
            "{\n    run_contracts: [\n        {\n            logical_entry: \"demo::cli\",\n            physical_entry: \"demo_cli.mjs\",\n            invocation: \"node\",\n            validate: \"node demo_cli.mjs\"\n        }\n    ]\n}\n",
        )
        .expect("write contract");

    let options = RegistryPackOptions {
        artifact_dir: artifact.clone(),
        package_root: dir.path().to_path_buf(),
        registry: "jsr".to_string(),
        meta: PackMeta {
            name: "@scope/pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            license: Some("MIT".to_string()),
        },
        include_files: vec!["NOTICE".into(), "dist/**/docs/*.md".into()],
        flat_layout: true,
        layout: nyar_package_manager::ProjectLayout::neutral(),
    };
    let packed = pack_registry_artifact(&options).expect("pack");

    let unpack_dir = dir.path().join("unpacked-jsr");
    std::fs::create_dir_all(&unpack_dir).expect("unpack dir");
    unpack(&packed.tarball_data, &unpack_dir).expect("unpack");
    assert!(unpack_dir.join("docs/extra.md").is_file());
    assert!(unpack_dir.join("NOTICE").is_file());
    assert!(unpack_dir.join("jsr.json").is_file());
    assert!(!unpack_dir.join("package").is_dir());
}

#[test]
fn bin_name_from_logical_entry() {
    assert_eq!(bin_name_from_logical("demo::cli"), "cli");
    assert_eq!(bin_name_from_logical("demo::main"), "main");
}

#[test]
fn glob_match_supports_star_and_double_star() {
    assert!(glob_match("README.md", "README.md"));
    assert!(glob_match("docs/*.md", "docs/guide.md"));
    assert!(glob_match("dist/**/*.mjs", "dist/wasm32-node-unknown-wasm/demo.mjs"));
    assert!(!glob_match("docs/*.md", "docs/sub/guide.md"));
}
