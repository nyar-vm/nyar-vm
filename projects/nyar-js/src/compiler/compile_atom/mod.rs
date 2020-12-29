use super::*;

impl JsCompiler {
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
