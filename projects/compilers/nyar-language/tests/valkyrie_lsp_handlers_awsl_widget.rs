use nyar_language::valkyrie::lsp::handlers::awsl::widget::map_synthetic_span_to_file;

#[test]
fn map_synthetic_span_to_file_removes_prefix_and_adds_script_base() {
    let mapped = map_synthetic_span_to_file(120, 18, 26..30);
    assert_eq!(mapped, 128..132);
}
