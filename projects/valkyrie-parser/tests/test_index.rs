use valkyrie_parser::{ast::ASTKind, ASTDump};

const INPUT: &str = r#"
a[1]

a   [
    2
    ]

a
[3]
"#;

#[test]
fn debug_index() -> std::io::Result<()> {
    let ast: ASTKind = ASTDump::parse(BYTES);
    ast.save(file!());
}
