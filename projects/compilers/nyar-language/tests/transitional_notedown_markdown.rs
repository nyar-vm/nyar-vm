use nyar_language::transitional::notedown::markdown::{format, parse};

#[test]
fn parses_highlight_extension() {
    let doc = parse("==highlighted==");
    assert!(!doc.blocks.is_empty());
}

#[test]
fn round_trip_heading() {
    let source = "# Title\n\nBody";
    let doc = parse(source);
    let formatted = format(&doc);
    assert!(formatted.contains("# Title"));
}
