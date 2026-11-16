//! Language-owned guest value exchange for the PE / Futamura substrate.
//!
//! Concrete language product logic lives in the `rusty-<lang>` guest crates. This module only
//! defines how the runner hands values in and out: every value stays owned by its language
//! behind an opaque handle, and interpreter failures surface as [`GuestError`] instead of being
//! flattened into a null value.

use std::{any::Any, collections::HashMap, fmt, sync::Arc};

/// A guest value owned by its language.
///
/// The substrate never interprets these values itself. Hosts may only ask for the owning
/// language, the language-local type name, and — for languages that define it — a scalar
/// reading. A failed scalar read returns `None` rather than a guessed default.
pub trait GuestValue: fmt::Debug + fmt::Display {
    /// Language id that owns this value.
    fn language(&self) -> &'static str;

    /// Language-local type name (`nil`, `table`, `string`, …).
    fn type_name(&self) -> &'static str;

    /// Scalar reading as `i64`, when the language value has one.
    fn as_i64(&self) -> Option<i64> {
        None
    }

    /// Scalar reading as `f64`, when the language value has one.
    fn as_f64(&self) -> Option<f64> {
        None
    }

    /// Scalar reading as `bool`, when the language value has one.
    fn as_bool(&self) -> Option<bool> {
        None
    }

    /// Downcast hook for the owning language adapter.
    fn as_any(&self) -> &dyn Any;
}

/// Shared handle to a language-owned guest value.
pub type GuestValueHandle = Arc<dyn GuestValue>;

/// Guest interpreter failure, kept structured instead of being turned into a null value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestError {
    /// Language whose interpreter failed.
    pub language: String,
    /// Language-local error message.
    pub message: String,
}

impl GuestError {
    /// Build an error for `language` from any displayable language error.
    pub fn new(language: impl Into<String>, message: impl fmt::Display) -> Self {
        Self { language: language.into(), message: message.to_string() }
    }
}

impl fmt::Display for GuestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.language, self.message)
    }
}

impl std::error::Error for GuestError {}

/// Guest program environment.
///
/// Values must come from the language of the interpreter that consumes them; a foreign handle
/// is a structured error, never a stringified substitute.
pub type GuestEnv = HashMap<String, GuestValueHandle>;

/// Function-pointer shape registered on [`crate::LegendVmRunner`].
///
/// Guests typically expose `evaluate_<lang>_source` in their `rusty-<lang>` crate; VM adapters
/// wrap those into this signature.
pub type GuestInterpretFn = fn(&str, &mut GuestEnv) -> Result<GuestValueHandle, GuestError>;

/// Thin, language-agnostic entry for “run this guest program”.
///
/// Prefer inherent APIs on concrete guests in their `rusty-<lang>` crate. This trait is the
/// substrate-side registration / dispatch surface only.
pub trait GuestInterpret {
    /// Canonical ids and aliases this guest answers to (e.g. `powershell`, `ps1`).
    fn language_ids(&self) -> &[&str];

    /// Interpret `source`, reading and writing the shared runtime environment.
    fn interpret(&self, source: &str, env: &mut GuestEnv) -> Result<GuestValueHandle, GuestError>;
}

/// [`GuestInterpret`] over a static id list and [`GuestInterpretFn`].
#[derive(Debug, Clone, Copy)]
pub struct FnGuestInterpret {
    /// Language ids / aliases.
    pub ids: &'static [&'static str],
    /// Interpret entry point.
    pub interpret: GuestInterpretFn,
}

impl GuestInterpret for FnGuestInterpret {
    fn language_ids(&self) -> &[&str] {
        self.ids
    }

    fn interpret(&self, source: &str, env: &mut GuestEnv) -> Result<GuestValueHandle, GuestError> {
        (self.interpret)(source, env)
    }
}
