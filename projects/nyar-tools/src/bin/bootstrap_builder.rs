use std::fs;
use std::io::Write;
use valkyrie_language::ast::Stmt;
use valkyrie_language::compiler;
use valkyrie_language::hir;
use valkyrie_language::lexer::lex;
use valkyrie_language::mir;
use valkyrie_language::parser::parse;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let files = vec![
        "examples/valkyrie-bootstrap/library/ast/ast.vk",
        "examples/valkyrie-bootstrap/library/ast/lexer.vk",
        "examples/valkyrie-bootstrap/library/ast/parser.vk",
        "examples/valkyrie-bootstrap/library/hir/hir.vk",
        "examples/valkyrie-bootstrap/library/hir/transformer.vk",
        "examples/valkyrie-bootstrap/library/lir/lir.vk",
        "examples/valkyrie-bootstrap/library/lir/backends/wasm.vk",
        "examples/valkyrie-bootstrap/library/lir/backends/jvm.vk",
        "examples/valkyrie-bootstrap/library/lir/backends/clr.vk",
        "examples/valkyrie-bootstrap/library/lir/backends/llvm.vk",
        "examples/valkyrie-bootstrap/library/lir/backends/native.vk",
        "examples/valkyrie-bootstrap/library/mir/mir.vk",
        "examples/valkyrie-bootstrap/library/mir/compiler.vk",
        "examples/valkyrie-bootstrap/test/call_macro.vk",
        "examples/valkyrie-bootstrap/binary/vcc/main.vk",
    ];

    let mut all_stmts = Vec::new();

    for path in files {
        println!("Compiling {}...", path);
        let src = fs::read_to_string(path)?;
        let toks_with_line = lex(&src).map_err(|e| format!("{:?}", e))?;
        let toks: Vec<valkyrie_language::lexer::Token> = toks_with_line.into_iter().map(|(t, _)| t).collect();
        let stmts = parse(&toks).map_err(|e| format!("{:?}", e))?;
        all_stmts.extend(stmts);
    }

    println!("Building HIR...");
    let hir = hir::build_hir(&all_stmts).map_err(|e| format!("{:?}", e))?;
    
    println!("Lowering to MIR...");
    let mir = mir::lower_hir_to_mir(&hir);

    println!("Converting to emit stmts...");
    let mut stmts_for_emit: Vec<Stmt> = Vec::new();
    for c in &mir.classes {
        stmts_for_emit.push(Stmt::ClassDef(c.name.clone(), c.fields.clone()));
    }
    for e in &mir.enums {
        stmts_for_emit.push(e.clone());
    }
    for t in &mir.traits {
        stmts_for_emit.push(Stmt::TraitDef(t.name.clone(), t.methods.clone()));
    }
    for im in &mir.impls {
        let methods: Vec<Stmt> = im
            .methods
            .iter()
            .map(|m| Stmt::FuncDef(m.name.clone(), m.args.clone(), m.body.clone()))
            .collect();
        stmts_for_emit.push(Stmt::ImplDef(
            im.trait_name.clone(),
            im.class_name.clone(),
            methods,
        ));
    }
    for f in &mir.functions {
        stmts_for_emit.push(Stmt::FuncDef(
            f.name.clone(),
            f.args.clone(),
            f.body.clone(),
        ));
    }
    stmts_for_emit.extend(mir.main.into_iter());

    println!("Compiling to Bytecode...");
    let module = compiler::compile(&stmts_for_emit).map_err(|e| format!("{:?}", e))?;

    let bytes = module.encode();
    let out_path = "examples/valkyrie-bootstrap/target/vcc.nyarc";
    fs::create_dir_all("examples/valkyrie-bootstrap/target")?;
    fs::write(out_path, bytes)?;
    println!("Written to {}", out_path);

    Ok(())
}
