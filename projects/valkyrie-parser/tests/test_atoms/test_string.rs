use super::*;

const CHARACTERS: &str = r#"
'';
"'";
'"';
«"'»;
‹'"›;
"#;

#[test]
fn debug_characters() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(CHARACTERS);
    ast.save("tests/test_atoms/debug_characters.yaml")
}

const ESCAPES: &str = r#"
"\n"
"#;

#[test]
fn debug_escapes() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(ESCAPES);
    ast.save("tests/test_atoms/debug_escapes.yaml")
}

const MULTILINE: &str = r#"
null
true
false
"#;

#[test]
fn debug_multiline() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(MULTILINE);
    ast.save("tests/test_atoms/debug_multiline.yaml")
}
