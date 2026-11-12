//! Migrated from `src/cache/artifact.rs` (`tests`).

use std::fs;

use nyar_emitter::DriverCompileReport;
use rusty_valkyrie::legion::cache::*;

use tempfile::tempdir;

#[test]
fn artifact_hash_changes_when_manifest_changes() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("main.v");
    let manifest = dir.path().join("legion.von");
    fs::write(&source, "micro main() -> i64 { return 0 }\n").unwrap();
    fs::write(&manifest, "target: clr\n").unwrap();
    let first = compute_artifact_hash(&[source.clone()], &[manifest.clone()], "clr", false, false, &Default::default(), None).unwrap();
    fs::write(&manifest, "target: wasm\n").unwrap();
    let second = compute_artifact_hash(&[source], &[manifest], "clr", false, false, &Default::default(), None).unwrap();
    assert_ne!(first, second);
}

#[test]
fn toolchain_fingerprint_carries_compiler_build_identity() {
    let build_id = rusty_valkyrie::legion::cache::compiler_build_id();
    assert!(!build_id.trim().is_empty(), "编译器构建身份不得为空");
    let fingerprint = toolchain_fingerprint();
    assert!(fingerprint.contains(&format!("compiler={build_id}")), "工具链指纹必须携带构建身份：{fingerprint}");
    // 消费者（N-API manifest view）从指纹提取身份必须与构建身份一致。
    assert_eq!(compiler_build_id_from_fingerprint(&fingerprint), Some(build_id));
}

#[test]
fn compiler_build_id_extraction_distinguishes_same_version_builds() {
    // 同版本不同构建：仅 `compiler=` 段不同（dirty 内容摘要不同）也必须提取出不同身份。
    let base = toolchain_fingerprint();
    let prefix = base.split(";compiler=").next().expect("fingerprint carries compiler segment");
    let build_a = format!("{prefix};compiler=abcdef123456-dirty000000000001");
    let build_b = format!("{prefix};compiler=abcdef123456-dirty000000000002");
    assert_eq!(compiler_build_id_from_fingerprint(&build_a), Some("abcdef123456-dirty000000000001"));
    assert_eq!(compiler_build_id_from_fingerprint(&build_b), Some("abcdef123456-dirty000000000002"));
    // 旧构建指纹缺少 `compiler=` 段或段值为空：提取失败返回 None，消费者不得伪造身份。
    assert_eq!(compiler_build_id_from_fingerprint("legion=0.1.0;identity=3;bytecode=7"), None);
    assert_eq!(compiler_build_id_from_fingerprint(&format!("{prefix};compiler=")), None);
}

#[test]
fn cache_ignores_unreported_output_files() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("old.mjs"), "stale").unwrap();
    let bundle = collect_build_bundle(dir.path(), &DriverCompileReport::default()).unwrap();
    assert!(bundle.files.is_empty());
}
