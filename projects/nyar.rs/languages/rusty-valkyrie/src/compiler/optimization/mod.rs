#![doc = include_str!("readme.md")]

mod closed_class;

pub use closed_class::{
    ClosedClassOptimizations, ClosedClassOptimizer, DeadCodeEliminationAnalyzer, MethodInlineAnalyzer, OptimizationResult,
    StackAllocationAnalyzer, StackAllocationRejectionReason, WitnessEliminationPass,
};
