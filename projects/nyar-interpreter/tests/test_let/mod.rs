use super::*;

const ATOMS: &str = r#"
let i = 1;
"#;

#[test]
fn debug_atoms() -> Result<()> {
    dump_value(ATOMS, "tests/test_let/debug_atoms.yaml")
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
