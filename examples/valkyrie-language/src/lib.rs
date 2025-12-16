use crate::ast::Stmt;
use crate::lexer::{lex, Error};
use crate::parser::parse;
use nyar_vm::bytecode::format::NyarcModule;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod ast;
pub mod compiler;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;

fn build_stmts_for_emit(src: &str) -> Result<Vec<Stmt>, Error> {
    let toks_with_line = lex(src)?;
    let toks: Vec<crate::lexer::Token> = toks_with_line.into_iter().map(|(t, _)| t).collect();
    let ast = parse(&toks)?;
    let hir = hir::build_hir(&ast)?;
    let mir = mir::lower_hir_to_mir(&hir);
    let mut stmts_for_emit: Vec<Stmt> = Vec::new();
    for c in &mir.classes {
        stmts_for_emit.push(Stmt::ClassDef(c.name.clone(), c.fields.clone()));
    }
    for e in &mir.enums {
        println!("DEBUG: emitting enum {:?}", e);
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
    println!("DEBUG: mir.main len: {}", mir.main.len());
    stmts_for_emit.extend(mir.main.into_iter());
    println!("DEBUG: stmts_for_emit len: {}", stmts_for_emit.len());
    Ok(stmts_for_emit)
}

pub fn compile_text_to_module_with_timestamp(
    src: &str,
    timestamp: u64,
) -> Result<NyarcModule, Error> {
    let stmts_for_emit = build_stmts_for_emit(src)?;
    let mut module = compiler::compile(&stmts_for_emit)?;
    module.timestamp = timestamp;
    Ok(module)
}

pub fn compile_text_to_module(src: &str) -> Result<NyarcModule, Error> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    compile_text_to_module_with_timestamp(src, ts)
}
