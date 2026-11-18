//! Nyar 工作区缓存门面。
//!
//! 内容寻址的制品桶、哈希与标记根发现由 Boost 的 `package-cache` 提供；本 crate 只做再导出。
#![deny(missing_docs)]

pub use package_cache::{CacheError as WorkspaceCacheError, Result, WorkspaceCache, combined_hash, file_hash, files_hash, resolve_marker_root, sanitize_bucket_name};
