use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StringTemplateBuilder {
    pub inner: Vec<ASTNode>,
    pub buffer: String,
    pub range: Span,
}

impl StringTemplateBuilder {
    pub fn new(span: Span) -> StringTemplateBuilder {
        Self { inner: vec![], buffer: "".to_string(), range: span }
    }
    pub fn push_string(&mut self, s: &str) {
        self.buffer.push_str(s)
    }
    pub fn push_escape_x(&mut self, s: &str) {}
    pub fn push_escape_u(&mut self, s: &str) {}
    pub fn push_escape_uu(&mut self, s: &str) {}

    pub fn push_expression(&mut self) {
        self.finish()
    }
    pub fn push_variable(&mut self) {
        self.finish()
    }
}

impl StringTemplateBuilder {
    pub fn build(&mut self) -> ASTNode {
        self.finish();
        ASTNode { kind: ASTKind::StringTemplate(self.inner.clone()), span: self.range }
    }
    fn finish(&mut self) {}
}
