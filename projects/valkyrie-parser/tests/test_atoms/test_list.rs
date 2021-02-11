use super::*;

const LISTS: &str = r#"
[];
();
(1, );
[1, 2, 3, ];
[(), [], [[]]];
([], (), (()))
"#;

#[test]
fn debug_list() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(LISTS);
    ast.save("tests/test_atoms/debug_list.yaml")
}

const MAPS: &str = r#"
[];
[1: 2, 3: 4];
[a: 1, z: 26];
"#;

#[test]
fn debug_dict() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(MAPS);
    ast.save("tests/test_atoms/debug_dict.yaml")
}

const SLICE: &str = r#"
[1, 2, 3][1];
(1, 2, 3)[1:2];
[1, 2, 3][1:2:1];
"#;

#[test]
fn debug_slice() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(SLICE);
    ast.save("tests/test_atoms/debug_slice.yaml")
}
