#[test]
fn byte_range_contains() {
    let r = ByteRange::new(5..10);
    assert!(r.contains(5));
    assert!(r.contains(9));
    assert!(!r.contains(10));
}

#[test]
fn error_node_flagged() {
    let node = SyntaxNode::error(1, 0..3, "unexpected");
    assert!(node.is_error);
}

#[test]
fn leading_trivia_collected() {
    let node = SyntaxNode::new(
        1,
        0..20,
        vec![
            SyntaxElement::Token(SyntaxToken::trivia(TriviaKind::LineComment, "# hi\n", 0..5)),
            SyntaxElement::Token(SyntaxToken { kind: 1, trivia_kind: None, text: "let".into(), span: ByteRange::new(5..8) }),
        ],
    );
    assert_eq!(node.leading_trivia_text(), "# hi\n");
}
