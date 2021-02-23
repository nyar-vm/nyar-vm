use super::*;

mod test_list;
mod test_number;
mod test_string;
mod test_symbol;

const SPECIALS: &str = r#"
null
true
false
"#;

#[test]
fn debug_specials() {
    let _: ASTKind = ASTDump::parse(SPECIALS);
}
