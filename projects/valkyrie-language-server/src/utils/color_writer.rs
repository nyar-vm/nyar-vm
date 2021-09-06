use std::fmt::Write;

#[derive(Default)]
pub struct ColoredWriter {
    pub buffer: String,
    pub schema: ColorSchema,
}

pub struct ColorSchema {
    pub bad: String,
    pub keyword: String,
    pub class: String,
    pub attribute: String,
}

impl Default for ColorSchema {
    fn default() -> Self {
        Self {
            bad: "F44747".to_string(),
            keyword: "C679DD".to_string(),
            class: "E5C17C".to_string(),
            attribute: "57B6C2".to_string(),
        }
    }
}

impl ColoredWriter {
    pub fn write_text(&mut self, text: &str) {
        self.buffer.push_str(text);
    }
    pub fn write_newline(&mut self) {
        self.buffer.push_str("<br/>");
    }
    pub fn write_bad(&mut self, text: &str) {
        self.buffer.push_str(&format!("<span style=\"color: #{}\">{}</span>", self.schema.bad, text));
    }
    pub fn write_keyword(&mut self, text: &str) {
        self.buffer.push_str(&format!("<span style=\"color: #{}\">{}</span>", self.schema.keyword, text));
    }
    pub fn write_modifiers<T: AsRef<str>>(&mut self, text: &[T]) {
        for modifier in text.iter() {
            self.buffer.push_str(&format!("<span style=\"color: #{}\">{}</span>", self.schema.keyword, modifier.as_ref()));
            self.buffer.push_str(" ");
        }
    }
    pub fn write_class(&mut self, text: &str) {
        write!(self.buffer, "<span style='color:#{color}'>{text}</span>", color = self.schema.class, text = text).ok();
    }
    pub fn write_attribute(&mut self, text: &str) {
        write!(self.buffer, "<span style='color:#{color}'>{text}</span>", color = self.schema.attribute, text = text).ok();
    }
    pub fn finish(self) -> String {
        self.buffer
    }
}
