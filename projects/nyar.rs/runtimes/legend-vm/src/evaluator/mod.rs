//! Guest language adapters for the legend VM runner.
//!
//! **Target shape:** thin bridges that delegate interpret to `rusty-<lang>` guest
//! crates (see `bash.rs` / `lua.rs` / `powershell.rs`). Registration uses the language-neutral
//! [`crate::guest::GuestInterpretFn`] seam — no concrete language ASTs here, and values stay
//! owned by their language.

mod bash;
mod c;
mod lua;
mod powershell;
mod tcl;

pub use crate::guest::{
    FnGuestInterpret, GuestEnv, GuestError, GuestInterpret, GuestInterpretFn, GuestValue, GuestValueHandle,
};

pub use bash::{bash_host_value, evaluate_bash_script};
pub use c::{c_host_value, evaluate_c_script};
pub use lua::{evaluate_lua_script, lua_host_value};
pub use powershell::{evaluate_powershell_script, powershell_host_value};
pub use tcl::{evaluate_tcl_script, tcl_host_value};

/// AST/tree evaluator interface (optional object-shaped hook over [`GuestInterpretFn`]).
pub trait TreeEvaluator {
    /// Evaluate source in the given environment.
    fn evaluate(&mut self, source: &str, env: &mut GuestEnv) -> Result<GuestValueHandle, GuestError>;
}
