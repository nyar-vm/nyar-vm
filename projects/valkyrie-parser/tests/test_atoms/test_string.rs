use super::*;

const CHARACTERS: &str = r#"
'';
"";
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
'\x24'
'\u03D6'
'\U000024'
'\n';
'\a';
'\
';
"#;

#[test]
fn debug_escapes() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(ESCAPES);
    ast.save("tests/test_atoms/debug_escapes.yaml")
}

const MULTILINE: &str = r#"
json"""
{
    x: 1
    y: 2
}
"""
"#;

#[test]
fn debug_multiline() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(MULTILINE);
    ast.save("tests/test_atoms/debug_multiline.yaml")
}
