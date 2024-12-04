use nyar_analyzer::highlight::{HighlightKind, HighlightSpan, merge_spans};

#[test]
fn semantic_overrides_same_start() {
    let lexical = vec![HighlightSpan::new(HighlightKind::Identifier, 0..3), HighlightSpan::new(HighlightKind::Keyword, 4..7)];
    let semantic = vec![HighlightSpan::new(HighlightKind::TypeIdentifier, 0..3)];
    let merged = merge_spans(lexical, semantic);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].kind, HighlightKind::TypeIdentifier);
    assert_eq!(merged[1].kind, HighlightKind::Keyword);
}
