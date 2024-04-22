//! Path helpers for cache buckets and workspace discovery.

use std::path::{Path, PathBuf};

/// Sanitize a bucket name for filesystem use (`-` and spaces become `_`).
pub fn sanitize_bucket_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '-' | ' ' => '_',
            other => other,
        })
        .collect()
}

/// Walk upward from `start` looking for a file named `marker`.
///
/// Returns the directory containing the marker when found.
pub fn resolve_marker_root(start: impl AsRef<Path>, marker: &str) -> Option<PathBuf> {
    let mut current = start.as_ref().to_path_buf();
    if current.is_file() {
        current = current.parent()?.to_path_buf();
    }
    loop {
        if current.join(marker).is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
