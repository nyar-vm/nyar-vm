extern crate nyar_valkyrie;

use nyar_valkyrie::get_ast;

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
    let ast = get_ast(NUMBERS);
    ast.save("tests/debug_numbers.json");
}

#[test]
fn debug_bytes() {
    let ast = get_ast(BYTES);
    ast.save("tests/debug_bytes.json");
}
