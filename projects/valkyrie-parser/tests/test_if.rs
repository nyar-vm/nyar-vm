use valkyrie_parser::{ast::ASTKind, ASTDump};

const INPUT: &str = r#"
if a {1}
if a {1} else {2}
if a {1} else if b {2}
if a {1} else if b {2} else {3}
if a {1} else if b {2} else if c {3}
"#;

#[test]
fn debug_if() {
    let ast = get_ast(INPUT);
    ast.save("tests/debug_if.yaml");
}
