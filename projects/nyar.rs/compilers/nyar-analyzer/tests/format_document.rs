use nyar_analyzer::{
    format::{Document, FormatOptions},
    *,
};

fn options(width: usize) -> FormatOptions {
    FormatOptions { indent_width: 4, ensure_trailing_newline: false, max_width: width, ..Default::default() }
}

#[test]
fn group_stays_flat_when_fits() {
    let document = Document::braces(Document::text("a: 1").append(Document::text(",")).append(Document::line()).append(Document::text("b: 2")));
    assert_eq!(document.render(&options(80)), "{a: 1, b: 2}");
}

#[test]
fn group_breaks_when_narrow() {
    let body = Document::join(Document::text(",").append(Document::line()), [Document::text("a: 1"), Document::text("b: 2")]);
    let document = Document::braces(body);
    let out = document.render(&options(10));
    assert!(out.contains('\n'), "{out}");
    assert!(out.starts_with('{'));
    assert!(out.ends_with('}'));
}

#[test]
fn compact_forces_flat() {
    let body = Document::join(Document::text(",").append(Document::line()), [Document::text("a: 1"), Document::text("b: 2")]);
    let document = Document::braces(body);
    assert_eq!(document.render_compact(&options(1)), "{a: 1, b: 2}");
}

#[test]
fn hardline_always_breaks() {
    let document = Document::text("a").append(Document::hardline()).append(Document::text("b"));
    assert_eq!(document.render_compact(&options(80)), "a\nb");
}
