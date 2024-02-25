use std::fmt::Write;

use crate::{
    CanonicalImport, CanonicalWasi, ExternalFunction, wasi_types::ComponentDefine, WasiInstance, WasiParameter, WasiResource,
};

mod for_instance;

pub struct WastEncoder<'a, W> {
    pub(crate) source: &'a CanonicalWasi,
    pub(crate) writer: W,
    indent: usize,
    indent_text: &'static str,
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn new(source: &'a CanonicalWasi, writer: W) -> Self {
        Self { source, writer, indent: 0, indent_text: "    " }
    }
    pub fn with_indent_text(self, text: &'static str) -> Self {
        Self { indent_text: text, ..self }
    }
}

impl<'a, W: Write> Write for WastEncoder<'a, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.writer.write_str(s)
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn encode(&mut self) -> std::fmt::Result {
        write!(self.writer, "(component ${}", self.source.name)?;
        self.indent();
        for import in &self.source.imports {
            import.component_define(self)?;
        }
        self.dedent(1);
        Ok(())
    }
    pub fn indent(&mut self) {
        self.indent += 1;
        self.newline().ok();
    }
    pub fn dedent(&mut self, end: usize) {
        self.indent -= 1;
        self.newline().ok();
        for _ in 0..end {
            self.write_char(')').ok();
        }
    }
    pub fn newline(&mut self) -> std::fmt::Result {
        self.write_str("\n")?;
        let range = (0..self.indent).into_iter();
        for _ in range {
            let indent = self.indent_text.as_ref();
            self.writer.write_str(indent)?;
        }
        Ok(())
    }
}

pub fn encode_id(id: &str) -> String {
    let mut alloc = String::with_capacity(id.len() + 1);
    alloc.push('$');
    make_kebab(id, &mut alloc);
    alloc
}

pub fn encode_kebab(id: &str) -> String {
    let mut alloc = String::with_capacity(id.len() + 2);
    alloc.push('"');
    make_kebab(id, &mut alloc);
    alloc.push('"');
    alloc
}

fn make_kebab(input: &str, buffer: &mut String) {
    for c in input.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | ':' | '@' | '/' => buffer.push(c),
            _ => buffer.push('-'),
        }
    }
}
