use super::*;

const ATOM: &str = r#"
a.b();
"#;

#[test]
fn debug_bytes() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(ATOM);
    ast.save("tests/test_expr/debug_data.yaml")
}

const DOT_CALL: &str = r#"
a::b::c
a::b::c()

a::b.c
a::b.c()

a.b::c
a.b::c()

a.b.c
~~ a.(b.c)
a.b.c()
a.b().c()
a().b().c()
"#;

#[test]
fn debug_dot_call() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(DOT_CALL);
    ast.save("tests/test_expr/debug_dot_call.yaml")
}
