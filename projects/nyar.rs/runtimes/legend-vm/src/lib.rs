//! Language-agnostic PE / Futamura interpreter substrate (GraalVM / Truffle-like).
//!
//! Core (`compiler` / [`guest`]) must stay free of concrete language product logic. Guest
//! languages live in their `examples/rusty-<lang>` crates and plug in via thin evaluator
//! adapters that implement the [`guest`] seam. See crate `readme.md` and
//! `projects/host-script-languages.md`.

#![warn(missing_docs)]

pub mod compiler;
pub mod evaluator;
pub mod guest;
pub mod runner;

pub use guest::{FnGuestInterpret, GuestEnv, GuestError, GuestInterpret, GuestInterpretFn, GuestValue, GuestValueHandle};
pub use runner::LegendVmRunner;
