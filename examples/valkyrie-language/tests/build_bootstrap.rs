use std::fs;
use std::path::Path;
use valkyrie_language::compile_text_to_module;

#[test]
fn build_bootstrap_nyarc() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let base = Path::new(manifest_dir)
        .parent()
        .unwrap()
        .join("valkyrie-bootstrap");
    let files = [
        "library/ast/ast.vk",
        "library/ast/lexer.vk",
        "library/ast/parser.vk",
        "library/hir/hir.vk",
        "library/hir/transformer.vk",
        "library/mir/mir.vk",
        "library/mir/compiler.vk",
        "library/lir/lir.vk",
        "library/lir/backends/wasm.vk",
        "library/lir/backends/jvm.vk",
        "library/lir/backends/clr.vk",
        "library/lir/backends/llvm.vk",
        "library/lir/backends/native.vk",
        "binary/vcc/main.vk",
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
    let out_dir = base.join("target");
    let _ = fs::create_dir_all(&out_dir);
    let out_path = out_dir.join("bootstrap.nyarc");
    fs::write(&out_path, &bytes).expect("write nyarc");
    println!("Wrote {:?}", out_path);
}
