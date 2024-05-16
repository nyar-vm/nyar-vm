use nyar_language::{
    notedown::{NotedownHtmlOptions, render},
    transitional::notedown::markdown,
};

#[test]
fn renders_heading_with_id() {
    let doc = markdown::parse("# Hello\n\nWorld");
    let result = render(&doc, &NotedownHtmlOptions::legion_doc(""));
    assert!(result.html.contains("<h1 id=\"hello\">"));
    assert!(result.html.contains("<p>"));
}

#[test]
fn rewrites_md_links() {
    let doc = markdown::parse("[link](./foo.md)");
    let result = render(&doc, &NotedownHtmlOptions::legion_doc(""));
    assert!(result.html.contains("href=\"./foo.html\""));
}

#[test]
fn renders_code_block_wrapper() {
    let doc = markdown::parse("```rust\nlet x = 1;\n```");
    let result = render(&doc, &NotedownHtmlOptions::legion_doc(""));
    assert!(result.html.contains("vp-code-block"));
    assert!(result.html.contains("vp-code-lang"));
}
