pub use chomsky_extract::IKunTree;
use nyar_gc::{MarkContext, Trace};
use oak_core::Language;
use serde::{Deserialize, Serialize};

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

impl Trace for QualifiedName {
    fn trace(&self, _ctx: &mut MarkContext) {
        // Strings are not GC-managed by nyar-gc, but this satisfies the Trace trait
    }
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.parts.join("::"))
    }
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

pub trait NyarFrontend: Default {
    type Language: Language;

    fn parse(&self, source: &str) -> Result<<Self::Language as Language>::TypedRoot, NyarError>;
    fn lower(&self, ast: &<Self::Language as Language>::TypedRoot) -> Result<IKunTree, NyarError>;
}
