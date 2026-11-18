#![deny(missing_docs)]

//! Odin VM consumer facade.
//!
//! # Layering
//!
//! - The bytecode engine substrate, the host JSON boundary and the workload-intent helpers all live
//!   in Boost **`interpreter-vm-ovm`** and are re-exported here unchanged.
//! - This crate keeps no implementation of its own; product consumers keep the `odin-vm` name.
//!
//! # Naming
//!
//! Crate package: `odin-vm`. Rust import path: `odin_vm`.

pub use interpreter_vm_ovm::*;
pub use interpreter_vm_ovm::workload_json::{WorkloadJsonError, parse_workload_intent_json, snapshot_gc_evidence};
