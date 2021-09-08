const TABLES: &str = r#"

"#;

#[test]
fn debug_table() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(TABLES);
    ast.save("tests/literal/table.vk")
}
