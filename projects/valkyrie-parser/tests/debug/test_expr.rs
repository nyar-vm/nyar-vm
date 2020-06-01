extern crate nyar_valkyrie;

use nyar_valkyrie::get_ast;

const INPUT: &str = r#"
+1+2*3^-4!!
1 +++ 2
1 * (2 + 3)
"#;

#[test]
fn debug_expr() {
    let ast = get_ast(INPUT);
    ast.save("tests/debug_expr.json");
}

const PARENTHESES: &str = r#"
[1 + 2];
[1 + 2, 3,];
[1 + 2, 3, []];
[1 + 2, 3, [4,5]];
[1 + 2, 3, [4,5,[]]];

a[1 + 1];
a[1 + 1] + 1;
a[[1,2,3]]
"#;

#[test]
fn debug_list_or_slice() {
    let ast = get_ast(PARENTHESES);
    ast.save("tests/debug_list_or_slice.json");
}

const BRACKETS: &str = r#"
a(1)[2]
b[1](2)
"#;

#[test]
fn debug_expr_brackets() {
    let ast = get_ast(BRACKETS);
    ast.save("tests/debug_expr_brackets.json");
    assert_eq!(0, 1)
}
