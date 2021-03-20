use super::*;

const ATOM: &str = r#"
a.b();
"#;

#[test]
fn debug_bytes() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(ATOM);
    ast.save("tests/test_expr/debug_data.yaml")
}

const SPECIALS: &str = r#"
null
true
false
"#;

#[test]
fn debug_specials() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(SPECIALS);
    ast.save("tests/test_atoms/debug_specials.yaml")
}
