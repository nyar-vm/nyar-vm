use super::*;



const LISTS: &str = r#"
[
    [],
    [1],
    [2,],
    [3, true, []]
]
"#;

#[test]
fn debug_lists() -> Result<()> {
    dump_value(LISTS, "tests/test_exprs/debug_lists.yaml")
}

const NUMBERS_WITH_HANDLERS: &str = r#"
123456
123456i
1234.56im
1234.56cm
"#;

#[test]
fn debug_number_handlers() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(NUMBERS_WITH_HANDLERS);
    let mut engine = NyarEngine::default();
    let out = engine.evaluate(&ASTNode::from(ast));
    println!("{:#?}", out);
    Ok(())
}

const SPECIALS: &str = r#"
null
true
false
"#;

#[test]
fn debug_specials() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(SPECIALS);
    let mut engine = NyarEngine::default();
    let out = engine.evaluate(&ASTNode::from(ast))?;
    println!("{}", out);
    Ok(())
}
