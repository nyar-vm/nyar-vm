use dashmap::DashMap;

use nyar_error::SourceSpan;

use crate::{builtin::texts::StringID, utils::xx_id, ValkyrieString};
use dashmap::mapref::one::Ref;
use shredder::{
    marker::{GcDrop, GcSafe},
    Scan, Scanner,
};
use smallvec::SmallVec;
use std::{
    fmt::{Debug, Display, Formatter},
    hash::{Hash, Hasher},
    sync::LazyLock,
};
use valkyrie_ast::{IdentifierNode, NamePathNode};

const MISSING_ID: &'static str = "<MISS_VALKYRIE_ID>";

/// A unique identifier used to query the valkyrie object
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ValkyrieID {
    hash: u64,
}

unsafe impl GcSafe for ValkyrieID {}
unsafe impl GcDrop for ValkyrieID {}
unsafe impl Scan for ValkyrieID {
    fn scan(&self, _: &mut Scanner<'_>) {}
}

pub struct ValkyrieUniverse {
    pub(crate) paths: DashMap<ValkyrieID, ValkyrieIDEntry>,
    pub(crate) texts: DashMap<StringID, Vec<u8>>,
}

pub static VALKYRIE_UNIVERSE: LazyLock<ValkyrieUniverse> =
    LazyLock::new(|| ValkyrieUniverse { paths: Default::default(), texts: Default::default() });

pub static STRING_POOL: LazyLock<ValkyrieUniverse> =
    LazyLock::new(|| ValkyrieUniverse { paths: Default::default(), texts: Default::default() });

struct ValkyrieIDEntry {
    name: SmallVec<StringID, 4>,
    span: SourceSpan,
}

impl ValkyrieUniverse {
    pub fn get_name(&self, id: ValkyrieID) -> Option<Ref<StringID, Vec<u8>>> {
        let id = self.paths.get(&id)?;
        let id = id.name.last()?;
        let id = self.texts.get(id)?;
        Some(id)
    }
}

impl Hash for ValkyrieIDEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.span.hash(state);
        for s in &self.name {
            s.hash(state)
        }
    }
}
impl Display for ValkyrieID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Debug for ValkyrieID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match VALKYRIE_UNIVERSE.paths.get(self) {
            Some(s) => {
                for (i, text) in s.value().name.iter().enumerate() {
                    if i != 0 {
                        f.write_str("∷")?
                    }
                    Debug::fmt(text, f)?
                }
            }
            None => f.write_str(MISSING_ID)?,
        }
        Ok(())
    }
}

impl From<NamePathNode> for ValkyrieIDEntry {
    fn from(value: NamePathNode) -> Self {
        let span = match value.path.as_slice() {
            [.., last] => last.span,
            _ => Default::default(),
        };
        ValkyrieIDEntry::from_iter(value.path.into_iter().map(|s| s.name)).with_span(span)
    }
}
impl From<IdentifierNode> for ValkyrieIDEntry {
    fn from(value: IdentifierNode) -> Self {
        let mut vec = SmallVec::with_capacity(1);
        vec.push(StringID::new(value.name));
        ValkyrieIDEntry { name: vec, span: value.span }
    }
}

impl FromIterator<String> for ValkyrieIDEntry {
    #[must_use]
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        Self { name: iter.into_iter().map(StringID::new).collect(), span: Default::default() }
    }
}

impl ValkyrieIDEntry {
    #[must_use]
    pub fn with_span(self, span: SourceSpan) -> Self {
        Self { span, ..self }
    }
    pub fn finish(self) -> ValkyrieID {
        let hash = xx_id(&self);
        VALKYRIE_UNIVERSE.paths.insert(ValkyrieID { hash }, self);
        ValkyrieID { hash }
    }
}

impl ValkyrieID {
    pub fn new<S>(s: S) -> Self
    where
        S: IntoIterator<Item = String>,
    {
        ValkyrieIDEntry::from_iter(s).finish()
    }

    /// Set the define location
    pub fn with_location(self, span: SourceSpan) -> Self {
        todo!()
    }
    /// Get the names of the file
    pub fn name(&self) -> ValkyrieString {
        ValkyrieString { ptr: VALKYRIE_UNIVERSE.get_name(*self) }
    }
    pub fn full_name(&self) -> &[String] {
        todo!()
    }
    pub fn namespace(&self) -> &[String] {
        todo!()
    }
}
