extern crate nyar_valkyrie;
use nyar_valkyrie::{get_statements,get_ast};

const INPUT: &str = r#"
if a {}
if a {} else {}
if a {} else if b {}
"#;

#[test]
fn debug() {
    get_statements(INPUT);
    assert_eq!(0, 1)
}

#[test]
fn debug2() {
    get_ast(INPUT);
    assert_eq!(0, 1)
}