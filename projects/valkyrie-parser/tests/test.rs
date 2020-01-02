extern crate nyar_valkyrie;
use nyar_valkyrie::{get_ast, get_statements};

const INPUT: &str = r#"
'';
"";
's';
"s";
""s"";
s's';
s"s";
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
