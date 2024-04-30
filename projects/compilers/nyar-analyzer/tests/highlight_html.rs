use nyar_analyzer::{
    highlight::{HighlightKind, HighlightSpan},
    *,
};

#[test]
fn renders_keyword_span() {
    let source = "let x";
    let spans = vec![HighlightSpan::new(HighlightKind::Keyword, 0..3)];
    let html = render_spans_html(source, &spans);
    assert!(html.contains("hl-keyword"));
    assert!(html.contains("let"));
    assert!(html.contains(" x") || html.ends_with("x") || html.contains(">x") || html.contains(" x"));
}
