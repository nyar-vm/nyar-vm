use super::*;
use nyar_hir::ast::LambdaFunction;

impl JsCompiler {
    pub(crate) fn compile_lambda(&mut self, input: &LambdaFunction) -> Result<()> {
        write!(self, "(function ")?;

        Ok(())
    }
}
