use nyar_optimizer::*;

#[test]
fn parse_nested_application() {
    let term = parse_term("core.add(core.mul(2, 3), 4)").unwrap();
    assert!(matches!(term, AlgebraicTerm::Apply { .. }));
}

#[test]
fn parse_literal_and_symbol() {
    assert_eq!(parse_term("42").unwrap(), AlgebraicTerm::literal(42));
    assert_eq!(parse_term("graphic.dot").unwrap(), AlgebraicTerm::symbol(qualified_from_dotted("graphic.dot")));
}
