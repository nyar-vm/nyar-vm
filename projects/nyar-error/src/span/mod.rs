use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
#[derive(Copy, Clone, Serialize, Deserialize)]

pub struct Span {
    /// span start offset
    pub start: u32,
    /// span end offset
    pub end: u32,
    /// same as ptr size, 0 refers to `<anonymous file>`
    pub file_id: u64,
}

impl Default for Span {
    fn default() -> Self {
        Self { start: 0, end: 0, file_id: 0 }
    }
}

impl Debug for Span {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.start, self.end)
    }
}
