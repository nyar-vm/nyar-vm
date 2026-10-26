//! Effect typing / constraint solver (split from monolithic effect_typing.rs).

mod model;
mod solver;

pub use model::*;
pub use solver::{ConstraintPropagator, ConstraintSolver, TraitBoundChecker};
