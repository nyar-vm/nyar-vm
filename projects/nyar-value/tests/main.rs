#[test]
fn ready() {
    println!("it, works!")
}

#[test]
#[ignore]
fn peg_codegen() -> QResult {
    let dir = current_dir()?.join("../valkyrie-parser/").canonicalize()?;
    Compile::file(dir.join("src/parser/swarm.peg")).destination(dir.join("src/swarm.rs")).format().run().unwrap();
    Ok(())
}
