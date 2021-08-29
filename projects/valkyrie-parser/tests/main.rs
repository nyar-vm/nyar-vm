use nyar_error::ReportResult;
use valkyrie_parser::ValkyrieParser;

#[test]
fn ready() {
    println!("it works!")
}

#[test]
fn test() -> ReportResult {
    let _ = ValkyrieParser::parse_file("tests/test_expr/infix3.vk")?;

    Ok(())
}
