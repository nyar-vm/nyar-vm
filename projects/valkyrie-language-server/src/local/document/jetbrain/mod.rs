use super::*;

impl TypedDocument {
    pub fn render_jetbrain(&self) -> (StatusCode, Json<ODocument>) {
        match self {
            TypedDocument::Nothing => (StatusCode::NOT_FOUND, Json::default()),
            TypedDocument::Keywords(s) => (StatusCode::OK, Json(ODocument { content: render_keywords(s) })),
        }
        (StatusCode::OK, Json(ODocument { content: doc }))
    }
}

pub struct ColoredWriter {
    pub buffer: String,
    pub schema: ColorSchema,
}

pub struct ColorSchema {
    pub keyword: String,
}

impl ColoredWriter {
    pub fn write_text(&mut self, text: &str) {
        self.buffer.push_str(text);
    }
    pub fn write_keyword(&mut self, text: &str) {
        write!(self.buffer, "<span style='color:{color}'>{text}</span>", self.schema.keyword, text).ok();
    }
}

fn render_keywords(s: &str) -> String {
    match s {
        "class" => "keyword class".to_string(),
        "trait" => "keyword trait".to_string(),
        "enum" => "keyword enum".to_string(),
        _ => format!("unknown keyword {}", s),
    }
}
