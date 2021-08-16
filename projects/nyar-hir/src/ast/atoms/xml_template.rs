use std::mem::take;

use nyar_error::{NyarError, Result};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct XMLTemplateBuilder {
    inner: Vec<ASTNode>,
    handler: String,
    buffer: String,
    text_start: u32,
    text_end: u32,
    range: Span,
}

impl Default for XMLTemplateBuilder {
    fn default() -> Self {
        Self {
            inner: vec![],
            handler: "".to_string(),
            buffer: "".to_string(),
            text_start: 0,
            text_end: 0,
            range: Default::default(),
        }
    }
}

impl XMLTemplateBuilder {
    pub fn new(span: Span) -> XMLTemplateBuilder {
        Self {
            inner: vec![],
            handler: "".to_string(),
            buffer: "".to_string(),
            text_start: span.start,
            text_end: span.start,
            range: span,
        }
    }
    pub fn push_buffer(&mut self, s: &str) {
        self.buffer.push_str(s)
    }
    pub fn push_handler(&mut self, handler: &str) {
        self.handler = handler.to_string()
    }
    pub fn has_handler(&self) -> bool {
        !self.handler.is_empty()
    }
    pub fn push_character(&mut self, s: &str, span: Span) -> Result<()> {
        self.text_end = span.end;
        let c = char::from_str(s)?;
        Ok(self.buffer.push(c))
    }
    pub fn push_unicode(&mut self, s: &str, span: Span) -> Result<()> {
        self.text_end = span.end;
        let mut cs = s.chars();
        cs.nth(2);
        cs.next_back();
        let c = char::from_u32(u32::from_str_radix(cs.as_str(), 16)?).ok_or(string_error(s, span))?;
        Ok(self.buffer.push(c))
    }
    pub fn push_escape(&mut self, s: &str, span: Span) -> Result<()> {
        self.text_end = span.end;
        let mut cs = s.chars();
        let c = cs.nth(1).ok_or(string_error(s, span))?;
        match c {
            '\n' => {}
            'n' => self.buffer.push('\n'),
            _ => self.buffer.push(c),
        }
        Ok(())
    }
    pub fn push_expression(&mut self, e: ASTNode) {
        self.finish();
        self.text_start = e.span.end;
        self.inner.push(e);
    }
    pub fn push_symbol(&mut self, s: Symbol, span: Span) {
        self.finish();
        self.text_start = span.end;
        self.inner.push(s.as_node(span));
    }
    fn finish(&mut self) {
        let text = ASTNode::string(
            take(&mut self.buffer),
            Span { start: self.text_start, end: self.text_end, file_id: self.range.file_id },
        );
        self.inner.push(text)
    }
}

fn string_error(t: &str, span: Span) -> NyarError {
    let msg = format!("`{}` does not a valid string element", t);
    return NyarError::syntax_error(msg).with_span(span);
}

impl XMLTemplateBuilder {
    pub fn as_node(&mut self) -> ASTNode {
        if self.inner.is_empty() {
            return ASTNode::string_handler(take(&mut self.buffer), &self.handler, self.range);
        }
        self.finish();
        ASTNode { kind: ASTKind::StringTemplate(self.inner.clone()), span: self.range }
    }
}
