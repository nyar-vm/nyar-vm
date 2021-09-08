use super::*;

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
    ast.save("tests/literal/debug_multiline.clj")
}

const XML: &str = r#"
<ul>
    <li>first item</li>
    <li>second item</li>
    <li>third item</li>
</ul>
"#;

#[test]
fn debug_xml() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(XML);
    ast.save("tests/literal/debug_xml.clj")
}
