use nyar_language::transitional::notedown::notedown::{MathType, NotedownInline, parse_inlines};

#[test]
fn parses_emphasis_and_code() {
    let inlines = parse_inlines("*hello* and `code`");
    assert!(inlines.iter().any(|i| matches!(i, NotedownInline::Emph(_))));
    assert!(inlines.iter().any(|i| matches!(i, NotedownInline::Code(_))));
}

#[test]
fn parses_math_inline_and_display() {
    let inline = parse_inlines("x = $a+b$");
    assert!(inline.iter().any(|i| matches!(i, NotedownInline::Math { math_type: MathType::Inline, .. })));
    let display = parse_inlines("$$E=mc^2$$");
    assert!(display.iter().any(|i| matches!(i, NotedownInline::Math { math_type: MathType::Display, .. })));
}

#[test]
fn parses_link() {
    let inlines = parse_inlines("[docs](guide.md)");
    assert!(inlines.iter().any(|i| matches!(i, NotedownInline::Link { .. })));
}
