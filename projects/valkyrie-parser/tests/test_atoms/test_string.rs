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
    ast.save("tests/test_atoms/debug_characters.clj")
}

const ESCAPES: &str = r#"
'\u{24}'
'\u{03D6}'
'\u{000024}'
'\n';
'\a';
'\
';
"#;

#[test]
fn debug_escapes() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(ESCAPES);
    ast.save("tests/test_atoms/debug_escapes.clj")
}

const MULTILINE: &str = r#"
''''
$x

${y}
${{}
'''';


json"""
{
    x: 1
    y: 2
}
""";
"#;

#[test]
fn debug_multiline() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(MULTILINE);
    ast.save("tests/test_atoms/debug_multiline.clj")
}
