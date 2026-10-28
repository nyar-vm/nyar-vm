#![doc = include_str!("readme.md")]
#![deny(missing_docs)]

extern crate self as rusty_valkyrie;

#[macro_use]
pub mod utilities;
pub mod asgard;
pub mod compiler;
pub mod legion;

/// Legacy re-exports for pre–TargetExecutionPlan integration suites.
#[doc(hidden)]
pub mod types {
    pub use crate::compiler::types::*;
}
#[doc(hidden)]
pub mod mir {
    pub use crate::compiler::mir::*;
    pub use crate::compiler::mir::ssa::*;
    pub use crate::compiler::mir::validation::*;
}
#[doc(hidden)]
pub mod hir {
    pub use crate::compiler::hir::*;
}
#[doc(hidden)]
pub mod frontend {
    pub mod ast {
        pub use oak_valkyrie::ast::*;
    }
}
#[doc(hidden)]
pub mod parser {
    pub use crate::compiler::syntax::oak::parse_source;
}

/// Re-export common test-facing types at crate root.
#[doc(hidden)]
pub use crate::api::ValkyrieCompiler;
#[doc(hidden)]
pub use crate::api::SourceID;
#[doc(hidden)]
pub use nyar_types::Identifier;
#[doc(hidden)]
pub use nyar_types::NamePath;
#[doc(hidden)]
pub use crate::compiler::hir::HirError;
#[doc(hidden)]
pub use crate::compiler::mir::MirLowerer;
#[doc(hidden)]
pub use crate::compiler::mir::validation::validate_semantic_module;

/// Stable compiler façade (no `nyar` re-export at crate root).
pub mod api {
    pub use crate::compiler::api::*;
}

/// 供外部集成测试引用的内部符号工具（不稳定，勿在正式代码中使用）。
#[doc(hidden)]
pub use compiler::emitter::backend_contract::interop::function_interop_contract;
/// 供外部集成测试引用的内部符号工具（不稳定，勿在正式代码中使用）。
#[doc(hidden)]
pub use compiler::hir::symbols::{stable_hir_function_name_path, stable_hir_function_symbol};
