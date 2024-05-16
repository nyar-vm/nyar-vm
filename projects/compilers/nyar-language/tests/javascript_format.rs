use nyar_language::{formatter::FormatOptions, javascript::format_javascript_source};

#[test]
fn formats_json() {
    let out = format_javascript_source("{ \"a\":1 }", "json", &FormatOptions::default()).unwrap();
    assert!(out.contains("\n"));
    assert!(out.contains("\"a\""));
}

#[test]
fn strips_trailing_space() {
    let out = format_javascript_source("const x = 1;  \n", "javascript", &FormatOptions::default()).unwrap();
    assert_eq!(out, "const x = 1;\n");
}
