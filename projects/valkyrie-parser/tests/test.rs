extern crate nyar_valkyrie;
use nyar_valkyrie::{get_ast, get_statements};

const INPUT: &str = r#"
123456
123456i
0xFF
0o77
0b11
0
0.
.0
0.0
true
false
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
