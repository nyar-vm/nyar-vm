pub mod test_atoms;
pub mod test_exprs;

use nyar_hir::ASTNode;
use nyar_interpreter::engine::NyarEngine;
use std::{fs::File, io::Write};
use valkyrie_parser::{ast::ASTKind, ASTDump, Result};

#[test]
fn ready() {
    println!("it works!")
}

fn dump_value(input: &str, path: &str) -> Result<()> {
    let ast: ASTKind = ASTDump::parse(input);
    let mut engine = NyarEngine::default();
    let value = engine.evaluate(&ASTNode::from(ast))?;
    let mut out = format!("{}", value);
    let mut file = File::create(path)?;
    file.write_all(out.as_bytes())?;
    Ok(())
}
