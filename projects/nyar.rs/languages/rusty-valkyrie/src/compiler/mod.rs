//! Valkyrie language compiler: syntax through emitter adaptation.
//!
//! Maintainer map: [`readme.md`](readme.md). Orchestration: `driver`; Nyar path: `emitter` (not Asgard UI `codegen`).

#![doc = include_str!("readme.md")]

pub mod api;
pub mod diagnostics;
pub mod driver;
pub mod emitter;
pub mod expansion;
pub mod hir;
pub mod input;
pub mod mir;
pub mod observation;
pub mod optimization;
pub mod semantic;
pub mod services;
pub mod syntax;
pub mod types;
