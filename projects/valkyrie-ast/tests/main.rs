use std::path::PathBuf;

use peginator_codegen::Compile;

use valkyrie_errors::{Url, ValkyrieResult};

#[test]
fn build_parser() -> ValkyrieResult {
    let root = PathBuf::from("../valkyrie-parser").canonicalize()?;
    let input = root.join("valkyrie.peg");
    let output = root.join("src/parser/valkyrie.rs");
    println!("Project root at: {}", Url::from_file_path(&root)?);
    println!("Input file at: {}", Url::from_file_path(&input)?);
    println!("Output file at: {}", Url::from_file_path(&output)?);
    Compile::file(input).destination(output).format().run_exit_on_error();
    Ok(())
}
