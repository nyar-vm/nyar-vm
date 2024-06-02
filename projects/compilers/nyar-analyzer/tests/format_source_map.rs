use nyar_analyzer::format::SourceMap;

#[test]
fn identity_map_roundtrip() {
    let mut map = SourceMap::new();
    map.push(0..10, 0..10);
    assert_eq!(map.orig_to_formatted(5), 5);
    assert_eq!(map.formatted_to_orig(5), 5);
}
