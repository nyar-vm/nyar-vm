use super::*;

const TUPLES: &str = r#"
();
(1, );
(1, 2, );
(true, (true,), ((true,(()))));
"#;

#[test]
fn debug_tuple() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(TUPLES);
    ast.save("tests/test_atoms/debug_tuple.clj")
}

const TABLES: &str = r#"
[];
[0, [], [[]]];
[1: 2, 3: 4];
[a: 1, z: 26];
["啊": 1, "吧": 2];
"#;

#[test]
fn debug_table() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(TABLES);
    ast.save("tests/test_atoms/debug_table.clj")
}
