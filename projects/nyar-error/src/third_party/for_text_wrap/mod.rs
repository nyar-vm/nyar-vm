use std::fmt::{Debug, Formatter};

use textwrap::indent;

pub fn debug_indent(value: impl Debug, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(&indent(&format!("{:?}", value), "    "))?;
    f.write_str("\n")
}
