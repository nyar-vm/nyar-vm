use nyar_language::python::format::normalize_python;

#[test]
fn normalizes_blanks() {
    assert_eq!(normalize_python("a = 1  \n\n\n\nb = 2\n"), "a = 1\n\n\nb = 2\n");
}
