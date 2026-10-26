//! Narrow compiler API surface for crate consumers. Legion/Asgard use `crate::compiler::*` internally.

pub use crate::compiler::{
    driver::validation::ControlFlowScheduler,
    emitter::{
        frontend_contract::{ConcretizeError, concretize_mir_function_types, concretize_type},
        pipeline::{
            CompilerArtifactReport, CompilerBuildContext, CompilerCompileEvidence, CompilerHostProviderBinding,
            compile_source_groups_to_artifacts, generated_source_hash, host_provider_binding_hash_for_bindings,
            semantic_closure_hash_for_groups,
        },
    },
    hir::{CaptureAnalyzer, function_body_contains_yield, lowering::ValkyrieCompiler},
    mir::{
        AggregateLayout, AggregateLayoutPlan, ArrayInitialization, FieldLayout, FlagsLayout, LayoutId, LayoutKey, MirBlock, MirBlockRef,
        MirConstant, MirDiagnostic, MirEffectKind, MirFunction, MirInstruction, MirLowerer, MirModule, MirOperand, MirOperation,
        MirStorageKind, MirTerminator, MirValue, MirValueOrigin, MirValueRef, SingletonInstancePlan, SumTypeLayout, SumVariantLayout,
        collect_singleton_instance_plans, compute_aggregate_layout_plan, layout_id_for_type, storage_kind_for_type,
    },
    semantic::closure::{
        ClosureGroupInput, DeclarationIndexEntry, DeclarationKey, DeclarationKind, EntryPointKind, GroupSemanticClosure, SemanticClosure,
        SemanticClosureError, plan_semantic_closure,
    },
    services::{
        format_provider::{ValkyriePrinterProvider, ValkyrieSourceFormatterProvider, register_format_providers},
        lint::{lint_valkyrie_source, lint_valkyrie_vx_source},
    },
    types::{Identifier, NamePath, QualifiedName, SourceID, SourceSpan},
};
