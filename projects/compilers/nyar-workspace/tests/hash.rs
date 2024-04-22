use nyar_workspace::{combined_hash, file_hash, files_hash};
use tempfile::tempdir;

#[test]
fn combined_hash_is_stable_and_order_sensitive() {
    let a = combined_hash(&["a", "b"]);
    let b = combined_hash(&["a", "b"]);
    let c = combined_hash(&["b", "a"]);
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.len(), 64);
}

#[test]
fn file_hash_same_content_same_hash() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a.v");
    std::fs::write(&path, b"hello").unwrap();
    assert_eq!(file_hash(&path).unwrap(), file_hash(&path).unwrap());
}

#[test]
fn files_hash_order_independent() {
    let dir = tempdir().unwrap();
    let p1 = dir.path().join("a.v");
    let p2 = dir.path().join("z.v");
    std::fs::write(&p1, b"one").unwrap();
    std::fs::write(&p2, b"two").unwrap();
    let forward = files_hash(&[p1.clone(), p2.clone()]).unwrap();
    let reverse = files_hash(&[p2, p1]).unwrap();
    assert_eq!(forward, reverse);
}
