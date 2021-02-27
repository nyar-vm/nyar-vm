use super::*;

const TUPLES: &str = r#"
();
(1, );
(1, 2, )
(true, (true,), ((true,(()))));
"#;

#[test]
fn debug_list() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(TUPLES);
    ast.save("tests/test_atoms/debug_tuple.yaml")
}

const TABLES: &str = r#"
[];
[0, [], [[]]];
[1: 2, 3: 4];
[a: 1, z: 26];
["啊": 1, "吧": 2];
"#;

#[test]
fn debug_dict() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(TABLES);
    ast.save("tests/test_atoms/debug_table.yaml")
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
