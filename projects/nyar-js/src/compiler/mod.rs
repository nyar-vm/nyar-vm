mod compile_atom;
mod writer;

use nyar_hir::{ASTKind, ASTNode};
use std::mem::take;

pub struct JsCompiler {
    buffer: String,
}

impl JsCompiler {
    pub fn compile(&mut self, input: &ASTNode) {
        match input.kind() {
            ASTKind::Nothing => {}
            ASTKind::Sequence(_) => {}
            ASTKind::LetBind(_) => {}
            ASTKind::ASTAtom(atom) => {
                self.compile_atom(atom);
            }
        }
    }
    pub fn finish(&mut self) -> String {
        take(&mut self.buffer)
    }
}
