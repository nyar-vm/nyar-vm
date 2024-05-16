use nyar_language::transitional::notedown::notedown::{NotedownLexer, NotedownTokenKind, parse_attr};

#[test]
fn tokenize_atx_header_and_code_fence() {
    let tokens = NotedownLexer::new("# Title\n\n```rust\nfn main() {}\n```").tokenize();
    assert!(tokens.iter().any(|t| matches!(t.kind, NotedownTokenKind::AtxHeader)));
    assert!(tokens.iter().any(|t| matches!(t.kind, NotedownTokenKind::CodeFenceStart)));
    assert!(tokens.iter().any(|t| matches!(t.kind, NotedownTokenKind::CodeFenceEnd)));
}

#[test]
fn parse_attr_extracts_id_and_class() {
    let attr = parse_attr("#intro .warning .rust");
    assert_eq!(attr.id, "intro");
    assert!(attr.classes.contains(&"warning".to_string()));
    assert!(attr.classes.contains(&"rust".to_string()));
    assert_eq!(attr.language(), "warning");
}
