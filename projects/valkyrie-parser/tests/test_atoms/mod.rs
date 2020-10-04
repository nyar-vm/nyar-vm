mod test_number;
mod test_string;

use super::*;

const SYMBOLS: &str = r#"
a;
a::b;
a::b::c;
我;
我::的;
我::的::库;
"#;

#[test]
fn debug_symbols() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(SYMBOLS);
    ast.save("tests/test_atoms/debug_symbols.yaml")
}
