use super::*;

mod test_data;
mod test_dict;
mod test_index;
mod test_infix;
mod test_unary;

const BRACKETS: &str = r#"
a(1)[2]
b[1](2)

Persion(20,"2",a, a: 2)

"#;

#[test]
fn debug_expr_brackets() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(BRACKETS);
    ast.save("tests/test_expr/debug_expr_brackets.yaml")
}

const DOT_CALL: &str = r#"
a::b::c
a::b::c()

a::b.c
a::b.c()

a.b::c
a.b::c()

a.b.c
// a.(b.c)
a.b.c()
a.b().c()
a().b().c()
"#;

#[test]
fn debug_dot_call() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(DOT_CALL);
    ast.save("tests/test_expr/debug_dot_call.yaml")
}
const BYTES: &str = r#"

"#;

#[test]
fn debug_bytes() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(BYTES);
    ast.save("tests/test_atoms/debug_bytes.yaml")
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
