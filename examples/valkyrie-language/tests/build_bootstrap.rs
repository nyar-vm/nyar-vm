use std::fs;
use std::path::Path;
use valkyrie_language::compile_text_to_module;

#[test]
fn build_bootstrap_nyarc() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let base = Path::new(manifest_dir)
        .parent()
        .unwrap()
        .join("valkyrie-bootstrap")
        .join("source");
    let files = [
        "ast.vk",
        "lexer.vk",
        "parser.vk",
        "compiler.vk",
        "main.vk",
    ];
    let mut src = String::new();
    for f in files.iter() {
        let p = base.join(f);
        let content = fs::read_to_string(&p).expect("read vk");
        src.push_str(&content);
        src.push_str("\n");
    }
    let module = compile_text_to_module(&src).expect("compile vk to module");
    let bytes = module.encode();
    let out_dir = Path::new("examples/valkyrie-bootstrap/target");
    let _ = fs::create_dir_all(out_dir);
    let out_path = out_dir.join("bootstrap.nyarc");
    fs::write(&out_path, &bytes).expect("write nyarc");
    println!("Wrote {:?}", out_path);
}
