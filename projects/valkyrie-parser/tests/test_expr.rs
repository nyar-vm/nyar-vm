extern crate nyar_valkyrie;
use nyar_valkyrie::get_ast;

const INPUT: &str = r#"
+1+2*3^-4!!
"#;

#[test]
fn debug_expr() {
    let ast = get_ast(INPUT);
    ast.save("tests/debug_expr.json");
}
