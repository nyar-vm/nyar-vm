use dashmap::DashMap;

/// Inline Cache (IC) for dynamic dispatch optimization.
pub struct InlineCache {
    /// Maps call site IDs to a single monomorphic target entry.
    pub entries: DashMap<u32, IcEntry>,
}

impl InlineCache {
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Records a successful dispatch in the cache.
    pub fn record(&self, call_site: u32, class_id: u32, target: *const u8) {
        self.entries.insert(call_site, IcEntry { class_id, target });
    }

    /// Looks up a cached target for a call site.
    pub fn lookup(&self, call_site: u32, class_id: u32) -> Option<*const u8> {
        self.entries.get(&call_site).and_then(|entry| {
            if entry.class_id == class_id {
                Some(entry.target)
            } else {
                None
            }
        })
    }
}

/// A single entry in the inline cache.
pub struct IcEntry {
    /// The class ID for which this entry is valid.
    pub class_id: u32,
    /// The entry point of the compiled method.
    pub target: *const u8,
}

unsafe impl Send for IcEntry {}
unsafe impl Sync for IcEntry {}
unsafe impl Send for InlineCache {}
unsafe impl Sync for InlineCache {}
