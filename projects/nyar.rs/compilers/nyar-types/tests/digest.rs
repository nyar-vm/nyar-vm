use nyar_types::digest::combined_content_hash;

#[test]
fn combined_content_hash_is_order_sensitive_and_stable() {
    let first = combined_content_hash(&["a", "b"]);
    let second = combined_content_hash(&["a", "b"]);
    let reversed = combined_content_hash(&["b", "a"]);
    assert_eq!(first, second);
    assert_ne!(first, reversed);
}
