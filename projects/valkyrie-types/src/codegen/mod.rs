use std::{
    collections::BTreeSet,
    fmt::{Arguments, Write},
    mem::take,
    path::PathBuf,
};

use crate::types::ValkyrieMetaType;

#[derive(Default)]
pub struct ValkyrieCodegen {
    buffer: String,
    root: PathBuf,
    indent: usize,
    prelude: BTreeSet<String>,
}

impl ValkyrieCodegen {
    pub fn generate(&mut self, meta: &ValkyrieMetaType) -> Result<String, std::fmt::Error> {
        writeln!(self, "namespace {};", meta.namespace("."))?;
        Ok(take(&mut self.buffer))
    }
    pub fn target_path(&self) -> PathBuf {
        self.root.join("src").join("lib.rs")
    }
}

impl Write for ValkyrieCodegen {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.buffer.write_str(s)
    }
    fn write_char(&mut self, c: char) -> std::fmt::Result {
        self.buffer.write_char(c)
    }
    fn write_fmt(self: &mut Self, args: Arguments<'_>) -> std::fmt::Result {
        self.buffer.write_fmt(args)
    }
}
