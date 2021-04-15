use super::*;

const INDEX: &str = r#"
a.1
a
    .1

a[1]

a   [
    2
    ]

a
[3,4]

a
[5]
[6]
"#;

#[test]
fn debug_index() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INDEX);
    ast.save("tests/test_expr/debug_index.clj")
}
