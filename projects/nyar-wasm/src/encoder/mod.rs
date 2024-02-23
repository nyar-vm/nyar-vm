use std::{
    fmt::{Display, Formatter, Write},
    sync::Arc,
};

pub struct WasiCanonical {
    component: Arc<str>,
}

struct WastEncoder<'a, W: Write> {
    source: &'a WasiCanonical,
    writer: W,
    indent: usize,
}

impl Default for WasiCanonical {
    fn default() -> Self {
        Self { component: Arc::from("Root") }
    }
}

impl<'a, W: Write> Write for WastEncoder<'a, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.writer.write_str(s)
    }
}
