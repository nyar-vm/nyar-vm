use crate::lexer::{lex, Error};
use crate::parser::parse;
use nyar_vm::bytecode::format::NyarcModule;
use crate::ast::Stmt;

pub mod ast;
pub mod compiler;
pub mod lexer;
pub mod parser;
pub mod hir;
pub mod mir;

pub fn compile_text_to_module(src: &str) -> Result<NyarcModule, Error> {
    let toks = lex(src)?;
    let ast = parse(&toks)?;
    let hir = hir::build_hir(&ast)?;
    let mir = mir::lower_hir_to_mir(&hir);
    // Reconstruct a lowered sequence of statements for emission
    let mut stmts_for_emit: Vec<Stmt> = Vec::new();
    for c in &mir.classes {
        stmts_for_emit.push(Stmt::ClassDef(c.name.clone(), c.fields.clone()));
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
    compiler::compile(&stmts_for_emit)
}
