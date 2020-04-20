extern crate nyar_valkyrie;

use nyar_valkyrie::get_ast;

const INPUT: &str = r#"
if a {1}
if a {1} else {2}
if a {1} else if b {2}
if a {1} else if b {2} else {3}
if a {1} else if b {2} else if c {3}
"#;

#[test]
fn debug_if() {
    let ast = get_ast(INPUT);
    ast.save("tests/debug_expr.json");
    assert_eq!(0, 1)
}
