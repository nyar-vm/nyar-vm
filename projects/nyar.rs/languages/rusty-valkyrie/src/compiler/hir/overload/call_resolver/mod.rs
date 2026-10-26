//! 调用解析核心：try_resolve_call / match_call_candidate 等。

mod constructors;
mod core;
mod helpers;
mod patterns;

pub use constructors::{match_singleton_method_candidate, try_resolve_nullary_constructor};
pub use core::match_call_candidate;
pub use helpers::try_resolve_constructor;

pub(crate) use constructors::{
    build_constructor_resolved_call, try_resolve_constructor_call, try_resolve_instance_method, try_resolve_qualified_free_function,
    try_resolve_singleton_method, try_resolve_type_static_method,
};
pub(crate) use core::{
    candidate_has_receiver_parameter, is_unresolved_variant_reference, match_intrinsic_builtin_candidate, overload_symbol_path,
    peel_explicit_generic_arguments, try_resolve_call,
};
pub(crate) use helpers::instantiate_candidate_for_context;
pub(crate) use patterns::{enrich_sum_variant_operation_facts, try_resolve_pattern_extractor, try_resolve_try_propagate_ok_variant};
