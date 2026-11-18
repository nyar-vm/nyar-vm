#![doc = include_str!("readme.md")]
#![deny(missing_docs)]

//! 优化门面：理论、会话与投影计划全部由 Boost `compiler-optimize` 所有。

pub use compiler_optimize::{
    AlgebraicTerm, ConstantInterpreter, CoreConstantInterpreter, EGraphHost, EGraphSnapshot, EGraphStats, FutamuraProjectionFamily,
    HostProjectionBoundary, ObjectAlgebraicBuilder, ObjectAlgebraicDimension, ObjectAlgebraicInterpreter, ObjectAlgebraicProgram,
    OptimizationRequest, OptimizationResult, OptimizationSession, ProjectionPlan, ProjectionPolicy, RewriteEquation, RewritePhase,
    RewriteRule, RewriteTheory, StructuredRewrite, TermPattern, TermRewrite, TheoryBundle, atom_pattern, builtin_core_rewrite_theory,
    builtin_core_term_rewrites, core_lit_name, filter_equations, filter_term_rewrites, parse_pattern, parse_term, qualified_from_dotted,
};
pub use compiler_types::ReferenceManagement;
