extern crate nyar_valkyrie;

use nyar_valkyrie::{get_ast, get_statements};

const BYTES: &str = r#"
0xFF
0o77
0b11
"#;

const NUMBERS: &str = r#"
0
0.
.0
123456.
.789
0.0
123456
123456i
1234.56im
1234.56cm
"#;

#[test]
fn debug_numbers() {
    get_ast(NUMBERS);
}

#[test]
fn debug_bytes() {
    get_ast(BYTES);
}
