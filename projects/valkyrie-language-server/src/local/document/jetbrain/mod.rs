use super::*;

impl TypedDocument {
    pub fn render_jetbrain(&self) -> Result<Json<ODocument>, StatusCode> {
        let content = match self {
            TypedDocument::Keywords(s) => render_keywords(s),
            TypedDocument::Operator(s) => render_operator(s),
            TypedDocument::Trait { namepath } => render_trait(namepath),
        };
        Ok(Json(ODocument { content }))
    }
}

fn render_keywords(keyword: &ValkyrieKeyword) -> String {
    let mut w = ColoredWriter::default();
    w.write_modifiers(&["keyword"]);
    w.write_attribute(keyword.name());
    w.write_newline();
    w.write_text(&keyword.document());
    w.finish()
}

fn render_operator(operator: &ValkyrieOperator) -> String {
    let mut w = ColoredWriter::default();
    w.write_modifiers(&["operator"]);
    w.write_attribute(operator.name());
    w.write_text(" ");
    w.write_attribute(operator.literal());
    w.write_newline();
    w.write_text("An operator is a symbol that tells the compiler to perform specific mathematical or logical manipulations.");
    w.finish()
}

fn render_trait(namepath: &[String]) -> String {
    let mut w = ColoredWriter::default();
    w.write_modifiers(&["namespace"]);
    w.write_text("std::categories");

    w.write_modifiers(&["public", "static", "trait"]);
    w.write_attribute(&namepath.join("."));
    w.write_newline();
    w.write_text("A trait is a collection of methods that are implemented by a type.");
    w.finish()
}
