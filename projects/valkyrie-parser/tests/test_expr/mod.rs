use super::*;

const UNARY: &str = r#"
+1;
2!;
+3!;
-+4!!;
"#;

#[test]
fn debug_unary() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(UNARY);
    ast.save("tests/test_expr/debug_unary.yaml")
}

const INFIX: &str = r#"
1 + 1;
2 ++ 2;
3 + + 3;
4 +++ 4
4 + ++ 4;
5 ++ + 5;
6 ++++ 6;
7 + +++ 7;
8 ++ ++ 8;
9 +++ + 9;
"#;

#[test]
fn debug_infix() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INFIX);
    ast.save("tests/test_expr/debug_infix.yaml")
}

const MIX_INFIX: &str = r#"
+1+2*3^-4!!;
"#;

#[test]
fn debug_mix_infix() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(MIX_INFIX);
    ast.save("tests/test_expr/debug_mix_infix.yaml")
}

// const LIST_OR_SLICE: &str = r#"
// [1 + 2];
// [1 + 2, 3,];
// [1 + 2, 3, []];
// [1 + 2, 3, [4,5]];
// [1 + 2, 3, [4,5,[]]];
//
// a[1 + 1];
// b[1 + 1] + 1;
// c[1,2,3];
// d[[1,2,3]];
// e[1:2:3,[1,2,3]];
// "#;
//
// #[test]
// fn debug_list_or_slice() {
//     let ast: ASTKind = ASTDump::parse(BYTES);
//     ast.save("tests/debug_list_or_slice.yaml");
// }
//
// const BRACKETS: &str = r#"
// a(1)[2]
// b[1](2)
//
// Persion(20,"2",a, a: 2)
//
// "#;
//
// #[test]
// fn debug_expr_brackets() {
//     let ast: ASTKind = ASTDump::parse(BYTES);
//     ast.save("tests/debug_expr_brackets.yaml");
// }
//
// const DOT_CALL: &str = r#"
// a::b::c
// a::b::c()
//
// a::b.c
// a::b.c()
//
// a.b::c
// a.b::c()
//
// a.b.c
// a.(b.c)
// a.b.c()
// a.b().c()
// a().b().c()
// "#;
//
// #[test]
// fn debug_dot_call() {
//     let ast: ASTKind = ASTDump::parse(BYTES);
//     ast.save("tests/debug_dot_call");
// }
