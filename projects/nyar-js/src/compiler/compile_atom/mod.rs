use super::*;
use nyar_hir::ast::ASTAtom;

impl JsCompiler {
    pub(crate) fn compile_atom(&mut self, input: &ASTAtom) {
        match input {
            ASTAtom::Boolean(v) => match v {
                true => {
                    self.buffer.push_str("true");
                }
                false => {
                    self.buffer.push_str("false");
                }
            },
            ASTAtom::String(_) => {}
        }
    }
}
