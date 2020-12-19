use super::*;
use std::fmt::{Arguments, Write};

impl Default for JsCompiler {
    fn default() -> Self {
        Self { buffer: "".to_string() }
    }
}

impl Write for JsCompiler {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.buffer.write_str(s)
    }

    fn write_char(&mut self, c: char) -> std::fmt::Result {
        self.buffer.write_char(c)
    }

    fn write_fmt(self: &mut Self, args: Arguments<'_>) -> std::fmt::Result {
        self.buffer.write_fmt(args)
    }
}
