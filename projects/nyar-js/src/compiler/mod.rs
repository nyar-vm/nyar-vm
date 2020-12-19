mod compile_atom;
mod compile_lambda;
mod writer;

use nyar_error::Result;
use nyar_hir::{ast::ASTAtom, ASTKind, ASTNode};
use std::{fmt::Write, mem::take};

pub struct JsCompiler {
    buffer: String,
}

impl JsCompiler {
    pub fn compile(&mut self, input: &ASTNode) -> Result<()> {
        match input.kind() {
            ASTKind::Nothing => {}
            ASTKind::Sequence(_) => {}
            ASTKind::LetBind(_) => {}
            ASTKind::ASTAtom(atom) => self.compile_atom(atom)?,
            ASTKind::LambdaFunction(lambda) => self.compile_lambda(lambda)?,
        }
        Ok(())
    }
    pub fn finish(&mut self) -> String {
        take(&mut self.buffer)
    }
}
