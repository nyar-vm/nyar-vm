use rusty_haskell::RustyHaskellFrontend;
use nyar_types::NyarFrontend;
use oak_haskell::ast::*;

#[test]
fn test_parse_and_translate() {
    let frontend = RustyHaskellFrontend::default();
    let source = r#"
module TestModule where
import Data.List
x = 42
"#;
    
    let ast = frontend.parse(source).expect("Failed to parse");
    // Depending on how HaskellParser works, this might be simplified
    assert!(ast.items.len() >= 1);
}

#[test]
fn test_function_definition() {
    let frontend = RustyHaskellFrontend::default();
    let source = r#"
add x y = 42
"#;
    
    let ast = frontend.parse(source).expect("Failed to parse");
    assert_eq!(ast.items.len(), 1);
    if let Item::Function(f) = &ast.items[0] {
        assert_eq!(f.name.name, "add");
        assert_eq!(f.equations.len(), 1);
        let eq = &f.equations[0];
        assert_eq!(eq.patterns.len(), 2);
        match &eq.body {
            Expression::Literal(Literal::Integer(42)) => {},
            _ => panic!("Expected literal 42, got {:?}", eq.body),
        }
    } else {
        panic!("Expected function");
    }
}

#[test]
fn test_lambda_expression() {
    let frontend = RustyHaskellFrontend::default();
    let source = r#"
f = \x -> x
"#;
    
    let ast = frontend.parse(source).expect("Failed to parse");
    assert_eq!(ast.items.len(), 1);
    if let Item::Function(f) = &ast.items[0] {
        assert_eq!(f.name.name, "f");
        let eq = &f.equations[0];
        match &eq.body {
            Expression::Lambda(pats, _) => {
                assert_eq!(pats.len(), 1);
            },
            _ => panic!("Expected lambda expression, got {:?}", eq.body),
        }
    } else {
        panic!("Expected function");
    }
}
