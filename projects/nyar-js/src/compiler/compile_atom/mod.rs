use super::*;

impl JsCompiler {
    pub(crate) fn compile_atom(&mut self, input: &ASTAtom) -> Result<()> {
        match input {
            ASTAtom::Boolean(v) => match v {
                true => self.buffer.push_str("true"),
                false => self.buffer.push_str("false"),
            },
            ASTAtom::String(v) => self.compile_raw_string(v),
            ASTAtom::Integer(v) => write!(self.buffer, "{}n", v)?,
            ASTAtom::Symbol(v) => self.compile_raw_symbol(v),
        }
        Ok(())
    }

    fn compile_raw_string(&mut self, input: &str) {
        self.buffer.push_str("\"");
        for c in input.chars() {
            match c {
                '\n' => self.buffer.push_str("\\n"),
                '\r' => self.buffer.push_str("\\r"),
                '\t' => self.buffer.push_str("\\t"),
                '\'' => self.buffer.push_str("\\'"),
                '\"' => self.buffer.push_str("\\\""),
                '\\' => self.buffer.push_str("\\\\"),
                _ => self.buffer.push(c),
            }
        }
        self.buffer.push_str("\"");
    }
    fn compile_raw_symbol(&mut self, input: &str) {
        for c in input.chars() {
            match c {
                '\n' => self.buffer.push_str("\\n"),
                _ => self.buffer.push(c),
            }
        }
    }
}