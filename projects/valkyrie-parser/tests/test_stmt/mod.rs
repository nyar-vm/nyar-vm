use super::*;

mod test_assign;
mod test_import;

const INPUT: &str = r#"
if a {1}
if a {1} else {2}
if a {1} else if b {2}
if a {1} else if b {2} else {3}
if a {1} else if b {2} else if c {3}
"#;

#[test]
fn debug_if() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INPUT);
    ast.save("tests/test_stmt/debug_if.yaml")
}
