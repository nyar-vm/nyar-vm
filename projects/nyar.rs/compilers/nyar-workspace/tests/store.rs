use std::path::PathBuf;

use nyar_workspace::WorkspaceCache;
use tempfile::tempdir;

#[test]
fn round_trip_put_then_get() {
    let dir = tempdir().unwrap();
    let cache = WorkspaceCache::open(dir.path().join(".cache"));
    let payload = b"hello-payload";
    cache.put("jvm-openjdk", "abc123", "ir", payload).unwrap();
    let got = cache.get("jvm-openjdk", "abc123", "ir").unwrap();
    assert_eq!(got.as_deref(), Some(payload.as_slice()));
}

#[test]
fn type_mismatch_is_miss() {
    let dir = tempdir().unwrap();
    let cache = WorkspaceCache::open(dir.path().join(".cache"));
    cache.put("bucket", "k1", "ir", b"data").unwrap();
    assert!(cache.get("bucket", "k1", "token").unwrap().is_none());
}

#[test]
fn different_buckets_are_isolated() {
    let dir = tempdir().unwrap();
    let cache = WorkspaceCache::open(dir.path().join(".cache"));
    cache.put("triple-a", "h", "ir", b"a").unwrap();
    assert!(cache.get("other-triple", "h", "ir").unwrap().is_none());
}

#[test]
fn entry_path_sanitizes_triple() {
    let cache = WorkspaceCache::open(PathBuf::from(".cache"));
    let path = cache.entry_path("wasm32-unknown-browser", "abcd");
    assert!(path.to_string_lossy().contains("wasm32_unknown_browser"));
    assert!(path.ends_with(format!("abcd.v{}.odin", acorn_odin::Odin_VERSION).as_str()));
}

#[test]
fn invalidate_bucket_and_all() {
    let dir = tempdir().unwrap();
    let cache = WorkspaceCache::open(dir.path().join(".cache"));
    cache.put("t1", "h1", "ir", b"1").unwrap();
    cache.put("t2", "h2", "ir", b"2").unwrap();
    cache.invalidate_bucket("t1").unwrap();
    assert!(cache.get("t1", "h1", "ir").unwrap().is_none());
    assert!(cache.get("t2", "h2", "ir").unwrap().is_some());
    cache.invalidate_all().unwrap();
    assert!(!cache.root.exists());
}

#[test]
fn persistence_across_instances() {
    let dir = tempdir().unwrap();
    let root = dir.path().join(".cache");
    {
        let cache = WorkspaceCache::open(&root);
        cache.put("m", "persist", "ir", &[0x63, 0x64, 0x65]).unwrap();
    }
    let cache2 = WorkspaceCache::open(&root);
    assert_eq!(cache2.get("m", "persist", "ir").unwrap().as_deref(), Some(&[0x63, 0x64, 0x65][..]));
}

/// 版本抬升后，残留在旧版本路径上的 v5 产物不得命中：缓存键按当前版本寻址。
#[test]
fn previous_version_path_is_not_probed() {
    let dir = tempdir().unwrap();
    let cache = WorkspaceCache::open(dir.path().join(".cache"));
    let stale = cache.entry_path("bucket", "key").with_file_name(format!("key.v{}.odin", acorn_odin::Odin_VERSION - 1));
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(&stale, b"stale-v5-artifact").unwrap();
    assert!(cache.get("bucket", "key", "ir").unwrap().is_none(), "旧版本路径不得被探测");
}

/// 即便 v5 产物被误放到当前版本路径，解码拒绝也必须表现为缓存未命中，
/// 不得把旧产物当作新合同产物复用。
#[test]
fn previous_version_artifact_at_current_path_is_a_miss() {
    let dir = tempdir().unwrap();
    let cache = WorkspaceCache::open(dir.path().join(".cache"));
    let path = cache.entry_path("bucket", "key");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let module = acorn_odin::OdinModuleData {
        version: acorn_odin::Odin_VERSION - 1,
        name: "ir".into(),
        constants: Vec::new(),
        functions: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        witness_entries: Vec::new(),
        code_bytes: b"old-contract-payload".to_vec(),
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    std::fs::write(&path, acorn_odin::encode_module(&module)).unwrap();
    assert!(cache.get("bucket", "key", "ir").unwrap().is_none(), "旧版本产物不得命中缓存");
}
