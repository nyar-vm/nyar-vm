#![deny(missing_docs)]

//! Odin VM consumer facade.
//!
//! # Layering
//!
//! - The bytecode engine substrate (module verification, interpreter loop, host boundary,
//!   runtime builtins) lives in Boost **`interpreter-vm-odin-vm`** and is re-exported here unchanged.
//! - This crate keeps only the product-side JSON bridge and workload-intent helpers, which
//!   depend on `serde_json` / `miette` and therefore must not move into the substrate.
//!
//! # Naming
//!
//! Crate package: `odin-vm`. Rust import path: `odin_vm`.

pub mod json_bridge;
pub mod workload_json;

pub use interpreter_vm_odin_vm::*;

pub use crate::workload_json::{WorkloadJsonError, parse_workload_intent_json, snapshot_gc_evidence};
