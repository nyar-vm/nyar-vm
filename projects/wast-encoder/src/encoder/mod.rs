use std::fmt::Write;

use crate::{CanonicalImport, WasiCanonical};

pub struct WastEncoder<'a, W> {
    pub(crate) source: &'a WasiCanonical,
    pub(crate) writer: W,
    indent: usize,
    indent_text: &'static str,
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn new(source: &'a WasiCanonical, writer: W) -> Self {
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

// (component $Root
//     (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
//         (export "error" (type (sub resource)))
//     ))
//     (alias export $wasi:io/error@0.2.0 "error" (type $io-error))
//
//     (type $stream-error (variant
//         (case "last-operation-failed" (own $io-error))
//         (case "closed")
//     ))
//     (type $stream-result (result (error $stream-error)))
//
//     (import "wasi:io/streams@0.2.0" (instance $wasi:io/streams@0.2.0
//         (export $output-stream "output-stream" (type (sub resource)))
//         (alias outer $Root $io-error (type $io-error))
//
//         (alias outer $Root $stream-error (type $stream-error0))
//         (export $stream-error "stream-error" (type (eq $stream-error0)))
//         (type $stream-result (result (error $stream-error)))
//
//         (export "[method]output-stream.blocking-write-and-flush"
//             (func (param "self" (borrow $output-stream)) (param "contents" (list u8)) (result $stream-result))
//         )
//     ))
//     (alias export $wasi:io/streams@0.2.0 "output-stream" (type $output-stream))
//     (alias export $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush" (func $output-stream.blocking-write-and-flush))
// )
impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn encode(&mut self) -> std::fmt::Result {
        write!(self.writer, "(component ${}", self.source.name)?;
        self.indent(true);
        for import in &self.source.imports {
            match import {
                CanonicalImport::Instance(instance) => self.encode_instance(instance)?,
            }
        }

        self.dedent(true);
        self.write_str(")")?;
        Ok(())
    }
    pub(crate) fn write_id(&mut self, id: &str) -> std::fmt::Result {
        write!(self.writer, "${}", id)
    }
    pub fn encode_id(&mut self, id: &str) -> String {
        let mut alloc = String::with_capacity(id.len() + 1);
        alloc.push('$');
        make_kebab(id, &mut alloc);
        alloc
    }
    pub fn encode_kebab(&mut self, id: &str) -> String {
        let mut alloc = String::with_capacity(id.len() + 2);
        alloc.push('"');
        make_kebab(id, &mut alloc);
        alloc.push('"');
        alloc
    }

    pub(crate) fn write_name(&mut self, id: &str) -> std::fmt::Result {
        write!(self.writer, "\"{}\"", id)
    }
    pub fn indent(&mut self, newline: bool) {
        self.indent += 1;
        if newline {
            self.newline().ok();
        }
    }
    pub fn dedent(&mut self, newline: bool) {
        self.indent -= 1;
        if newline {
            self.newline().ok();
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

fn make_kebab(input: &str, buffer: &mut String) {
    for c in input.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | ':' | '@' | '/' => buffer.push(c),
            _ => buffer.push('-'),
        }
    }
}