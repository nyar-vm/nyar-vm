//! frontend integration tests.

#[path = "../support/primitives.rs"]
pub(crate) mod primitives;

mod contract;
mod copy_discipline_value_fields;
mod diagnostics;
mod format;
mod hir;
mod mir;
mod semantic_closure;
