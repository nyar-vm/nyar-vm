use super::*;

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
true && false;
0 + 0.0 + 0cm;
"" ++ '';
"$x" ++ '${y}';
a >> a;
a in a;
a not in a;
a is A;
a is not a;
"#;

#[test]
fn debug_infix() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INFIX);
    ast.save("tests/test_expr/debug_infix.clj")
}

const INFIX2: &str = r#"
1 + 2 * 3;
(1+2) * 3;
1 + 2 * 3 + 4 * 5 * 6;
"#;

#[test]
fn debug_infix_order() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INFIX2);
    ast.save("tests/test_expr/debug_infix2.clj")
}

const MIX_INFIX: &str = r#"
1 > 2 > 3;
+1+2*3^-4!!;
"#;

#[test]
fn debug_mix_infix() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(MIX_INFIX);
    ast.save("tests/test_expr/debug_infix3.clj")
}
