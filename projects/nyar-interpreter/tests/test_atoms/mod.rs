use super::*;

const NUMBERS: &str = r#"
// 0.
// .0
// 123456.
// .789
0.0
0.02
42f32

0
2147483647i32
9223372036854775807i64
170141183460469231731687303715884105727i128
57896044618658097711785492504343953926634992332820282019728792003956564819967
"#;

#[test]
fn debug_numbers() -> Result<()> {
    dump_value(NUMBERS, "tests/test_atoms/debug_numbers.yaml")
}

const NUMBERS_WITH_HANDLERS: &str = r#"
123456
123456i
1234.56im
1234.56cm
"#;

#[test]
fn debug_number_handlers() -> Result<()> {
    dump_value(NUMBERS_WITH_HANDLERS, "tests/test_atoms/debug_number_handlers.yaml")
}

const SPECIALS: &str = r#"
null
true
false
"#;

#[test]
fn debug_specials() -> Result<()> {
    dump_value(SPECIALS, "tests/test_atoms/debug_specials.yaml")
}
