use super::*;

const ATOMS: &str = r#"
let i = 1;
"#;

#[test]
fn debug_atoms() -> Result<()> {
    dump_value(ATOMS, "tests/test_let/debug_atoms.yaml")
}

const COMPLEX: &str = r#"
let a, b: int;
let (a, b): (int, int)
let (a: int, b: int)
let mut a: int, = 1;
let (mut a: int, ) = 1;
let mut a, _, _ = (1,2,3)
"#;

#[test]
fn debug_complex() -> Result<()> {
    dump_value(COMPLEX, "tests/test_let/debug_complex.yaml")
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
