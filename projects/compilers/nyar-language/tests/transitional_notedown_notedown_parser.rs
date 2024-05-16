use nyar_language::transitional::notedown::notedown::{NotedownBlock, parse};

#[test]
fn parse_heading_and_paragraph() {
    let doc = parse("# Hello\n\nWorld");
    assert!(matches!(doc.blocks[0], NotedownBlock::Header { level: 1, .. }));
    assert!(matches!(doc.blocks[1], NotedownBlock::Para { .. }));
}

#[test]
fn parse_code_fence_with_language() {
    let doc = parse("```rust\nlet x = 1;\n```");
    if let NotedownBlock::CodeBlock { attr, text } = &doc.blocks[0] {
        assert_eq!(attr.language(), "rust");
        assert!(text.contains("let x"));
    }
    else {
        panic!("expected code block");
    }
}

#[test]
fn parse_gfm_table() {
    let doc = parse("| A | B |\n|---|---|\n| 1 | 2 |");
    assert!(matches!(doc.blocks[0], NotedownBlock::Table { .. }));
}
