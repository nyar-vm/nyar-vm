//! Minimal overload ranking helpers.
//!
//! 原始单文件 `overload.rs` 拆分为按职责的子模块：
//! - `diagnostics.rs`: 域/签名/候选/匹配类别/错误 + `rank` + `resolve_overload`
//! - `candidates.rs`: 候选构造（`build_*` / `collect_*` / `language_builtin_candidates`）
//! - `symbol_match.rs`: 符号匹配（`symbol_matches_*` / `candidate_*_matches` / `aggregate_constructor_matches_name`）
//! - `resolve_call.rs`: resolve_function_calls / resolve_block_calls / resolve_expr_calls / resolve_pattern_calls / resolve_arm_calls / validate_* 系列
//! - `call_resolver.rs`: `try_resolve_call` / `try_resolve_constructor_call` / `match_call_candidate` / `match_singleton_method_candidate` 等核心匹配逻辑
//! - `trait.rs`: `impl_nominal_type_name` / `impl_owner_identifier` 等 impl owner 辅助
//! - `substitution.rs`: `unify_call_type_binders` / `substitute_type_self` / `substitute_self_type` / `substitute_associated_equations` / `substitute_type_vars` / `member_call_argument_contexts` / `extract_singleton_type_name` 等代换辅助
//! - `types.rs`: 类型辅助 + 表达式类型推断（`infer_expr_type` / `infer_scrutinee_type` / `compute_call_match_kind` / `extract_callable_name` 等）
//! - `expr_position.rs`: `ExprPosition` enum（表达式位置标签）

pub(crate) mod call_resolver;
pub(crate) mod candidates;
pub(crate) mod diagnostics;
pub(crate) mod expr_position;
pub(crate) mod resolve_call;
pub(crate) mod substitution;
pub(crate) mod symbol_match;
pub(crate) mod r#trait;
pub(crate) mod types;

// ---- facade: 从各子模块 re-export 原始对外可见符号 -------------------------

// diagnostics 类型与入口
pub use diagnostics::{OverloadCandidate, OverloadDomain, OverloadMatchKind, OverloadResolutionError, resolve_overload};

// candidates
pub use candidates::collect_module_candidates;
pub(crate) use candidates::specialize_method_candidate;

// call_resolver 核心匹配
pub(crate) use call_resolver::instantiate_candidate_for_context;
pub use call_resolver::{match_call_candidate, match_singleton_method_candidate, try_resolve_constructor, try_resolve_nullary_constructor};

// substitution 对外可见的代换函数
#[doc(hidden)]
pub(crate) use substitution::{extract_singleton_type_name, substitute_associated_equations};
pub(crate) use substitution::{
    has_unresolved_generic_arguments, instantiate_trait_bound_candidate, instantiate_trait_method_candidate, member_call_argument_contexts,
    owner_type_generic_binders, substitute_self_type, substitute_trait_self, substitute_type_parameters, substitute_type_self,
    substitute_type_vars, unify_call_type_binders,
};

// trait helpers
pub(crate) use r#trait::impl_nominal_type_name;

// types
pub(crate) use types::{
    accepts_array_shape, canonical_extractor_callee, compute_call_match_kind, enum_apply_type, extract_callable_name, infer_expr_type,
    infer_scrutinee_type, is_empty_array_literal, is_text_type, normalize_local_field_chain, pattern_type_declaration,
    should_resolve_name_pattern_as_type, struct_apply_type, variant_payload_type,
};

// resolve_call 对外入口
pub use resolve_call::{resolve_hir_calls, validate_extractor_patterns};

// 从 symbol_match re-export 的函数（供 resolve_call / call_resolver 间跨模块调用）
pub(crate) use symbol_match::{
    aggregate_constructor_matches_name, candidate_extractor_path_matches, candidate_owned_method_matches, candidate_trait_method_matches,
    candidate_variant_constructor_matches, symbol_matches_imported_dependency_export, symbol_matches_language_builtin,
    symbol_matches_unqualified_free_call,
};

// call_resolver 里的 shim/辅助（被 resolve_call 直接引用）
pub(crate) use call_resolver::{
    build_constructor_resolved_call, candidate_has_receiver_parameter, enrich_sum_variant_operation_facts, overload_symbol_path,
    peel_explicit_generic_arguments, try_resolve_constructor_call, try_resolve_instance_method, try_resolve_singleton_method,
    try_resolve_type_static_method,
};
pub(crate) use resolve_call::bind_pattern_type;
