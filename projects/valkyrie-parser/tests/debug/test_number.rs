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
42f32
2147483647i32
9223372036854775807i64
170141183460469231731687303715884105727i128
57896044618658097711785492504343953926634992332820282019728792003956564819967
123456
123456i
1234.56im
1234.56cm
"#;

#[test]
fn debug_bytes() {
    let ast = get_ast(BYTES);
    ast.save("tests/debug_bytes.json");
}

#[test]
fn debug_numbers() {
    let ast = get_ast(NUMBERS);
    ast.save("tests/debug_numbers.json");
}
