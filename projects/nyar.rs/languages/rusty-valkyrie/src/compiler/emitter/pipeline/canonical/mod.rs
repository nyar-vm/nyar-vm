//! 将已完成语义解析的 MIR 生产为 CanonicalProgram。

mod errors;
mod fragments;
mod identities;
mod lower;
mod program;

pub use program::canonical_program_from_semantic_mir;
