use peginator_codegen::Compile;
use std::env::current_dir;
#[test]
fn ready() {
    println!("it, works!")
}

#[test]
#[ignore]
fn peg_codegen() {
    let dir = current_dir().unwrap().join("../valkyrie-parser/").canonicalize().unwrap();
    Compile::file(dir.join("src/parser/valkyrie.peg")).destination(dir.join("src/parser/valkyrie.rs")).format().run().unwrap();
}
