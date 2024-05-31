//! JavaScript / TypeScript analysis frontend (lint + module analysis).
//!
//! Source **formatting** for these languages belongs to Oak (`oak-typescript::formatter`);
//! the local whitespace-normalizing formatter was deleted rather than kept as a second
//! formatting engine.

#![doc = include_str!("readme.md")]

pub mod lint;
pub mod module;

pub use lint::{JavascriptLintIssue, JavascriptLintOptions, collect_javascript_files, lint_javascript_source};
pub use module::JavascriptModule;
