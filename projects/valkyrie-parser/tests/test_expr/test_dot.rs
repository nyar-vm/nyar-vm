use super::*;

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
    ast.save("tests/test_expr/debug_dot_call.clj")
}
