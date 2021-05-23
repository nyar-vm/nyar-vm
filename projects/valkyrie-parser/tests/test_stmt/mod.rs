use super::*;

mod test_assign;
mod test_control;
mod test_import;

const INPUT1: &str = r#"
if a {1}
if a {1} else {2}
if a {1} else if b {2}
if a {1} else if b {2} else {3}
if a {1} else if b {2} else if c {3}
if a {1} else if b {2} else if c {3} else {4}
"#;

#[test]
fn debug_if() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INPUT1);
    ast.save("tests/test_stmt/debug_if.clj")
}
