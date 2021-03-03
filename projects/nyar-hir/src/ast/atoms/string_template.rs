use std::mem::take;

use nyar_error::{NyarError, Result};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StringTemplateBuilder {
    pub inner: Vec<ASTNode>,
    pub buffer: String,
    pub text_start: u32,
    pub text_end: u32,
    pub range: Span,
}

impl StringTemplateBuilder {
    pub fn new(span: Span) -> StringTemplateBuilder {
        Self { inner: vec![], buffer: "".to_string(), text_start: span.start, text_end: span.start, range: span }
    }
    pub fn push_escape(&mut self, s: &str, span: Span) -> Result<()> {
        let mut cs = s.chars();
        let c = if s.starts_with("\\u") {
            cs.nth(2);
            cs.next_back();
            char::from_u32(u32::from_str_radix(cs.as_str(), 16)?)
        }
        else if s.starts_with("\\") {
            match cs.nth(1) {
                None => {}
                Some(_) => {}
            }
        }
        else {
            cs.nth(0)
        };
        self.buffer.push(c.ok_or(string_error(s, span))?);
        self.text_end = span.end;
        Ok(())
    }
    pub fn get_char() {}

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

impl StringTemplateBuilder {
    pub fn as_node(&mut self) -> ASTNode {
        if self.inner.is_empty() {
            return ASTNode::string(take(&mut self.buffer), self.range);
        }
        self.finish();
        ASTNode { kind: ASTKind::StringTemplate(self.inner.clone()), span: self.range }
    }
}
