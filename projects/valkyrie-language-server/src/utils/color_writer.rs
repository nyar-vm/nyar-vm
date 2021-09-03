use std::fmt::Write;

#[derive(Default)]
pub struct ColoredWriter {
    pub buffer: String,
    pub schema: ColorSchema,
}

pub struct ColorSchema {
    pub keyword: String,
    pub class: String,
}

impl Default for ColorSchema {
    fn default() -> Self {
        Self { keyword: "C679DD".to_string(), class: "E5C17C".to_string() }
    }
}

impl ColoredWriter {
    pub fn write_text(&mut self, text: &str) {
        self.buffer.push_str(text);
    }
    pub fn write_newline(&mut self) {
        self.buffer.push_str("<br/>");
    }
    pub fn write_keyword(&mut self, text: &str) {
        write!(self.buffer, "<span style='color:#{color}'>{text}</span>", color = self.schema.keyword, text = text).ok();
    }
    pub fn finish(self) -> String {
        self.buffer
    }
}
