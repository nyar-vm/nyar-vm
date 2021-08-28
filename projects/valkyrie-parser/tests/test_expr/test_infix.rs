use super::*;

const INFIX: &str = r#"
true && false;
0 + 0.0 + 0cm;
"" ++ '';
"$x" ++ '${y}';
a >> a;
"#;

#[test]
fn debug_infix() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INFIX);
    ast.save("tests/test_expr/debug_infix.clj")
}

const INFIX2: &str = r#"

"#;

#[test]
fn debug_infix_order() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INFIX2);
    ast.save("tests/test_expr/debug_infix2.clj")
}

const MIX_INFIX: &str = r#"

"#;

#[test]
fn debug_mix_infix() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(MIX_INFIX);
    ast.save("tests/test_expr/debug_infix3.clj")
}
