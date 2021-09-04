use super::*;

impl TypedDocument {
    pub fn render_jetbrain(&self) -> Result<Json<ODocument>, StatusCode> {
        let content = match self {
            TypedDocument::Keywords(s) => render_keywords(s),
            TypedDocument::Operator(s) => render_operator(s),
        };
        Ok(Json(ODocument { content }))
    }
}

fn render_keywords(keyword: &ValkyrieKeyword) -> String {
    let mut w = ColoredWriter::default();
    w.write_keyword("keyword");
    w.write_text(" ");
    w.write_attribute(keyword.name());
    w.write_newline();
    w.write_text("A keyword is a reserved word that cannot be used as an identifier.");
    w.finish()
}

fn render_operator(operator: &ValkyrieOperator) -> String {
    let mut w = ColoredWriter::default();
    w.write_keyword("operator");
    w.write_text(" ");
    w.write_attribute(operator.name());
    w.write_text(" ");
    w.write_attribute(operator.literal());
    w.write_newline();
    w.write_text("An operator is a symbol that tells the compiler to perform specific mathematical or logical manipulations.");
    w.finish()
}
