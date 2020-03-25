extern crate nyar_valkyrie;
use nyar_valkyrie::{get_ast, get_statements};

const INPUT: &str = r#"
1+1
"#;

#[test]
fn debug_expr() {
    get_ast(INPUT);
    assert_eq!(0, 1)
}
