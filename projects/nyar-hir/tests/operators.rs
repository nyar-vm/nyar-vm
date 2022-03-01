use valkyrie_ast::OperatorKind;
use valkyrie_errors::ValkyrieResult;

#[test]
fn infix() -> ValkyrieResult {
    assert_eq!(OperatorKind::parse_infix("+")?, OperatorKind::Add);
    assert_eq!(OperatorKind::parse_infix("-")?, OperatorKind::Subtract);
    Ok(())
}

#[test]
fn infix_in() -> ValkyrieResult {
    assert_eq!(OperatorKind::parse_infix("in")?, OperatorKind::In(true));
    assert_eq!(OperatorKind::parse_infix(" in   ")?, OperatorKind::In(true));
    //
    assert_eq!(OperatorKind::parse_infix(" ∈   ")?, OperatorKind::In(true));
    assert_eq!(OperatorKind::parse_infix(" !∈   ")?, OperatorKind::In(false));
    //
    assert_eq!(OperatorKind::parse_infix("not in")?, OperatorKind::In(false));
    assert_eq!(OperatorKind::parse_infix("  not      in   ")?, OperatorKind::In(false));
    Ok(())
}
