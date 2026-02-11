use serde::{Deserialize, Serialize};

#[cfg(feature = "gc")]
use nyar_gc::Trace;

pub mod errors;
pub use crate::errors::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QualifiedName {
    pub parts: Vec<String>,
}

impl QualifiedName {
    pub fn new(parts: Vec<String>) -> Self {
        Self { parts }
    }
    pub fn push(&mut self, part: String) {
        self.parts.push(part);
    }
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
    pub fn last(&self) -> Option<&String> {
        self.parts.last()
    }
}

impl FromIterator<String> for QualifiedName {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl From<&str> for QualifiedName {
    fn from(s: &str) -> Self {
        s.split("::").map(|s| s.to_string()).collect()
    }
}

impl From<String> for QualifiedName {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.parts.join("::"))
    }
}

#[cfg(feature = "gc")]
impl Trace for QualifiedName {
    fn trace(&self, _ctx: &mut nyar_gc::MarkContext<'_>) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct SourceLocation {
    pub source_id: u32,
    pub offset: u32,
}

impl SourceLocation {
    pub fn new(source_id: u32, offset: u32) -> Self {
        Self { source_id, offset }
    }
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source:{}:{}", self.source_id, self.offset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectInfo {
    pub name: QualifiedName,
    pub location: SourceLocation,
}

/// 模块定义
#[derive(Debug, Clone, Default)]
pub struct Module {
    pub id: ModuleId,
    pub name: String,
    pub chunks: Vec<u8>, // 占位符，实际可能更复杂
}

pub type ModuleId = usize;
