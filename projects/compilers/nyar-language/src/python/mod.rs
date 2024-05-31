//! Python analysis frontend (lint + module analysis).
//!
//! Source **formatting** for Python belongs to Oak; the local whitespace-normalizing
//! formatter was deleted rather than kept as a second formatting engine.
//! Product CLI: `panda`.

#![doc = include_str!("readme.md")]

pub mod lint;
pub mod module;

pub use lint::{PythonLintIssue, PythonLintOptions, collect_python_files, lint_python_path, lint_python_source};
pub use module::PythonModule;
