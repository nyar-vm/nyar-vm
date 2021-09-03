use crate::utils::ColoredWriter;

use super::*;

impl TypedDocument {
    pub fn render_jetbrain(&self) -> Result<Json<ODocument>, StatusCode> {
        match self {
            TypedDocument::Nothing => Err(StatusCode::NOT_FOUND),
            TypedDocument::Keywords(s) => Ok(Json(ODocument { content: render_keywords(s) })),
        }
    }
}

fn render_keywords(s: &str) -> String {
    let mut w = ColoredWriter::default();
    w.write_keyword("keyword");
    w.write_text(" ");
    match s {
        "class" => w.write_text("class"),
        "trait" => w.write_text("trait"),
        "union" => w.write_text("union"),
        _ => return format!("unknown keyword {}", s),
    };
    w.finish()
}
