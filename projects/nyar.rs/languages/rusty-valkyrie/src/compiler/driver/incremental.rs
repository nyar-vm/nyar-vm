//! Incremental invalidation skeleton (not populated yet).

/// Monotonic input revision for a compiler session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct InputRevision(u64);

/// Tracks which fact layers depend on which inputs (placeholder).
#[derive(Debug, Default)]
pub struct InvalidationGraph {
    revision: InputRevision,
}

impl InvalidationGraph {
    /// Creates an empty graph at revision zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bumps the session revision; consumers decide what to recompute.
    pub fn bump(&mut self) -> InputRevision {
        self.revision = InputRevision(self.revision.0 + 1);
        self.revision
    }

    /// Current revision token.
    pub fn revision(&self) -> InputRevision {
        self.revision
    }
}
