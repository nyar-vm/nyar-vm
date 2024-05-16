use nyar_language::transitional::notedown::notedown::{NotedownDocument, formatter::format};

#[test]
fn round_trip_heading() {
    let source = "# Title\n\nBody";
    let doc = NotedownDocument::parse(source);
    let formatted = format(&doc);
    let doc2 = NotedownDocument::parse(&formatted);
    assert_eq!(doc.blocks.len(), doc2.blocks.len());
}
