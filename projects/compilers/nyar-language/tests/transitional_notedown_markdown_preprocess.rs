use nyar_language::transitional::notedown::markdown::preprocess_markdown;

#[test]
fn converts_highlight_markers() {
    assert_eq!(preprocess_markdown("==foo=="), "[.highlight]foo[/]");
}

#[test]
fn leaves_unclosed_highlight() {
    assert_eq!(preprocess_markdown("==foo"), "==foo");
}
