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
    pub use crate::compiler::mir::{ssa::*, validation::*, *};
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
    /// 兼容旧测试：通过 frontend 模块访问 parser。
    pub fn parse_source(source: &str) -> Result<crate::compiler::syntax::oak::ValkyrieRoot, crate::compiler::syntax::oak::ParseError> {
        crate::compiler::syntax::oak::parse_source(source)
    }
}
#[doc(hidden)]
pub mod parser {
    pub use crate::compiler::syntax::oak::parse_source;
    /// 兼容旧测试。
    pub mod frontend {
        pub use crate::compiler::syntax::oak::parse_source;
    }
}

#[doc(hidden)]
pub use crate::api::SourceID;
/// Re-export common test-facing types at crate root.
#[doc(hidden)]
pub use crate::api::ValkyrieCompiler;
#[doc(hidden)]
pub use crate::compiler::hir::HirError;
#[doc(hidden)]
pub use crate::compiler::mir::MirLowerer;
#[doc(hidden)]
pub use crate::compiler::mir::validation::validate_semantic_module;
#[doc(hidden)]
pub use nyar_types::Identifier;
#[doc(hidden)]
pub use nyar_types::NamePath;
#[doc(hidden)]
pub use nyar_types::SourceSpan;

// MIR 类型（测试通过 glob 从 crate root 访问）
#[doc(hidden)]
pub use crate::compiler::mir::ssa::{FieldSpec, build_aggregate_layout, ensure_unite_tagged_layout};
#[doc(hidden)]
pub use crate::compiler::mir::{
    AggregateFieldIndex, AggregateLayout, AggregateLayoutPlan, FieldLayout, FlagsLayout, LayoutId, LayoutKey, MirBlock, MirBlockRef,
    MirConstant, MirDiagnostic, MirEffectKind, MirEntryContract, MirExportContract, MirExternalCallContract, MirField, MirFunction,
    MirInstruction, MirOperand, MirOperation, MirStorageKind, MirStruct, MirTerminator, MirValue, MirValueOrigin, MirValueRef,
    SingletonInstancePlan, SumTypeLayout, SumVariantLayout, compute_aggregate_layout_plan, layout_id_for_type, storage_kind_for_type,
};

// Semantic closure 类型（测试通过 glob 从 crate root 访问）
#[doc(hidden)]
pub use crate::compiler::semantic::closure::{
    ClosureGroupInput, DeclarationKind, SemanticClosure, SemanticClosureError, plan_semantic_closure,
};

// Compiler input 类型（测试通过 glob 从 crate root 访问）
#[doc(hidden)]
pub use nyar_types::compiler_input::{CompilerSourceFile, CompilerSourceGroup, CompilerSourceInput, GeneratedSourceGroup};

// Services：格式化 / lint / source_surface（旧测试通过 crate root 访问）
#[doc(hidden)]
pub use crate::compiler::emitter::frontend_contract::nyar_type::concretize_type;
#[doc(hidden)]
pub use crate::compiler::emitter::pipeline::{
    generated_source_hash, host_provider_binding_hash_for_bindings, semantic_closure_hash_for_groups,
};
#[doc(hidden)]
pub use crate::compiler::services::format_provider::register_format_providers;
#[doc(hidden)]
pub use crate::compiler::services::lint::lint_valkyrie_source;

// 旧测试遗留模块重导出（#[doc(hidden)]，非正式 API）
#[doc(hidden)]
pub mod source_surface {
    pub use crate::compiler::syntax::source_surface::*;
}
#[doc(hidden)]
pub mod text {
    pub use crate::utilities::text::*;
}
#[doc(hidden)]
pub mod highlight {
    pub use crate::compiler::services::highlight::*;
}
#[doc(hidden)]
pub mod format_provider {
    pub use crate::compiler::services::format_provider::*;
}

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
