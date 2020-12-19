use super::*;

impl JsCompiler {
    pub(crate) fn compile_lambda(&mut self, input: &ASTAtom) -> Result<()> {
        write!(self, "(function ")?;

        Ok(())
    }
}
