use super::*;

const NUMBERS: &str = r#"
0xFF
0o77
0b11
0a_b_c_d

0
-0.0
+0.0
0.0

+123456.0
-0.789
42f32
42_f32
42__f64

2147483647i32
9223372036854775807_i64
170141183460469231731687303715884105727__i128
57896044618658097711785492504343953926634992332820282019728792003956564819967

123456i
1234.56`啊`
1234.56cm
"#;

#[test]
fn debug_numbers() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(NUMBERS);
    ast.save("tests/test_atoms/debug_numbers.yaml")
}
