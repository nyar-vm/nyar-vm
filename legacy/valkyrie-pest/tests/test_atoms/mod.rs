use super::*;

mod test_number;
mod test_string;
mod test_symbol;
mod test_table;

const SPECIALS: &str = r#"
null
true
false
"#;

#[test]
fn debug_specials() {
    let _: ASTKind = ASTDump::parse(SPECIALS);
}
