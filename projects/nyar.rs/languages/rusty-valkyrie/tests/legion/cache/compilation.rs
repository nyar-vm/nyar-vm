//! Migrated from `src/cache/compilation.rs` (`tests`).
//!
//! IR 与 entry-slice 两层持久化共用一份 `tempdir` + `CompilationCache` fixture，
//! 不再为同一种「写入后读回」拆成两条独立用例。

use rusty_valkyrie::legion::cache::*;

use tempfile::tempdir;

fn open_cache(dir: &std::path::Path) -> CompilationCache {
    CompilationCache::open(dir.join(".cache"))
}

#[test]
fn compilation_cache_round_trips_ir_and_entry_slice() {
    let dir = tempdir().unwrap();
    let cache = open_cache(dir.path());

    let entry = IrCacheEntry { ir_kind: "artifact-set".into(), ir_data: vec![1, 2, 3], ir_hash: "h1".into(), canonical_triple: "clr".into() };
    cache.put_ir("mod", "clr", "h1", &entry).unwrap();
    let got = cache.try_get_ir("mod", "clr", "h1").expect("hit");
    assert_eq!(got.ir_data, vec![1, 2, 3]);
    assert_eq!(got.ir_kind, "artifact-set");

    let slice = EntrySliceCacheEntry {
        entry_name: "main".into(),
        reachable_functions: vec!["main".into(), "helper".into()],
        call_graph_hash: "cg".into(),
        canonical_triple: "clr".into(),
    };
    cache.put_entry_slice("main", "clr", "cg", &slice).unwrap();
    let got = cache.try_get_entry_slice("main", "clr", "cg").unwrap();
    assert_eq!(got.reachable_functions, vec!["main", "helper"]);
}
