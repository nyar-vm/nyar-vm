use super::*;

const LIST: &str = r#"
[1 + 2];
[1 + 2, 3,];
[1 + 2, 3, []];
[1 + 2, 3, [4,5]];
[1 + 2, 3, [4,5,[]]];
"#;

#[test]
fn debug_list() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(LIST);
    ast.save("tests/test_expr/debug_list.yaml")
}

const SLICE: &str = r#"
a[1 + 1];
b[1 + 1] + 1;
c[1,2,3];
d[[1,2,3]];
e[1:2:3,[1,2,3]];
"#;

#[test]
fn debug_slice() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(SLICE);
    ast.save("tests/test_expr/debug_slice.yaml")
}

const INDEX: &str = r#"
a[1]

a   [
    2
    ]

a
[3,4]

a
[5]
[6]
"#;

#[test]
fn debug_index() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INDEX);
    ast.save("tests/test_expr/debug_index.yaml")
}
