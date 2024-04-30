use nyar_analyzer::{
    format::{FormatBuffer, FormatOptions},
    *,
};

#[test]
fn buffer_indent_and_newline() {
    let options = FormatOptions { indent_width: 2, max_width: 80, ensure_trailing_newline: false, ..Default::default() };
    let mut buf = FormatBuffer::new(&options);
    buf.write("a");
    buf.newline();
    buf.indent();
    buf.write("b");
    buf.newline();
    buf.dedent();
    buf.write("c");
    assert_eq!(buf.finish(), "a\n  b\nc");
}
