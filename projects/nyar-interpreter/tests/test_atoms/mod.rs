use super::*;

const NUMBERS: &str = r#"
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

const BAD_DECIMAL: &str = r#"
0.
.0
123456.
.789
"#;

#[test]
#[should_panic]
fn errors_bad_decimal() {
    for input in BAD_DECIMAL.lines() {
        dump_value(input, "");
    }
}

const BYTES: &str = r#"
0b11
0o17
0x1F
"#;

#[test]
fn debug_bytes() -> Result<()> {
    dump_value(BYTES, "tests/test_atoms/debug_bytes.yaml")
}


const NUMBERS_WITH_HANDLERS: &str = r#"
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
