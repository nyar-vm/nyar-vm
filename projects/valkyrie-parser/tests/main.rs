// mod test_atoms;
// mod test_expr;
// mod test_stmt;
//
// use valkyrie_parser::{ast::ASTKind, ASTDump, Result};

use nyar_error::ReportResult;
use valkyrie_parser::ValkyrieParser;

#[test]
fn ready() {
    println!("it works!")
}

#[test]
fn test() -> ReportResult {
    let out = ValkyrieParser::parse_file("tests/basic.vk")?;

    Ok(())
}
