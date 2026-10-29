//! singleton 与 impl 方法的 MIR lowering。

use super::{
    callable_types::{CallableReturnTypes, impl_type_arguments_from_generics},
    function_lowering::lower_function_semantic,
    hir_identity::impl_owner_type_name,
    model::*,
    value_semantics::AggregateLayoutPlan,
};
use crate::compiler::types::hir::{HirModule, ValkyrieType};
use std::collections::BTreeMap;

/// 检查类型里是否还有未闭合的泛型 binder（`ValkyrieType::Generic(generic)` 且 `generic.binder.is_some()`）。
/// impl 方法的原始声明（参数/返回类型里还是 Generic binder）进不了 MIR，让 instantiated_callables 成为唯一来源。
fn contains_open_generic(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Generic(generic) => generic.binder.is_some(),
        ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(contains_open_generic),
        ValkyrieType::Apply(base, arguments) => contains_open_generic(base) || arguments.iter().any(contains_open_generic),
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => contains_open_generic(element),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
            items.iter().any(contains_open_generic)
        }
        ValkyrieType::Function(function) => {
            function.params.iter().any(|p| contains_open_generic(p)) || contains_open_generic(&function.return_type)
        }
        ValkyrieType::FixedArray { element, .. } => contains_open_generic(element),
        ValkyrieType::Associated(assoc) => {
            contains_open_generic(&assoc.base) || assoc.type_arguments.iter().any(contains_open_generic)
        }
        _ => false,
    }
}

pub(super) fn lower_singleton_method_functions(
    module: &HirModule,
    aggregate_layouts: &mut AggregateLayoutPlan,
    singleton_accessor_instances: &BTreeMap<String, nyar_types::ItemInstanceId>,
    callable_returns: &CallableReturnTypes,
    member_dispatch: &[super::generic_call_instantiation::MirDispatchEntry],
    host_runtime_callables: &BTreeMap<String, nyar_types::ItemInstanceId>,
    diagnostics: &mut Vec<MirDiagnostic>,
) -> Vec<MirFunction> {
    let mut functions = Vec::new();
    for singleton in &module.singletons {
        for method in singleton.methods.iter().filter(|method| {
            !method.is_abstract
                && !method.params.iter().any(|p| contains_open_generic(&p.ty))
                && !contains_open_generic(&method.return_type)
        }) {
            let mut mir_function = lower_function_semantic(
                module,
                method,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(ValkyrieType::Named(singleton.name.clone())),
                impl_type_arguments_from_generics(&singleton.generics),
                host_runtime_callables,
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", singleton.name, method.name);
            functions.push(mir_function);
        }
        if let Some(constructor) = &singleton.constructor {
            let mut mir_function = lower_function_semantic(
                module,
                constructor,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(ValkyrieType::Named(singleton.name.clone())),
                impl_type_arguments_from_generics(&singleton.generics),
                host_runtime_callables,
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", singleton.name, constructor.name);
            functions.push(mir_function);
        }
        if let Some(finalizer) = &singleton.finalizer {
            let mut mir_function = lower_function_semantic(
                module,
                finalizer,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(ValkyrieType::Named(singleton.name.clone())),
                impl_type_arguments_from_generics(&singleton.generics),
                host_runtime_callables,
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", singleton.name, finalizer.name);
            functions.push(mir_function);
        }
    }
    functions
}

pub(super) fn lower_impl_method_functions(
    module: &HirModule,
    aggregate_layouts: &mut AggregateLayoutPlan,
    singleton_accessor_instances: &BTreeMap<String, nyar_types::ItemInstanceId>,
    callable_returns: &CallableReturnTypes,
    member_dispatch: &[super::generic_call_instantiation::MirDispatchEntry],
    host_runtime_callables: &BTreeMap<String, nyar_types::ItemInstanceId>,
    diagnostics: &mut Vec<MirDiagnostic>,
) -> Vec<MirFunction> {
    let mut functions = Vec::new();
    for impl_block in &module.impls {
        let Some(type_name) = impl_owner_type_name(module, &impl_block.target)
        else {
            continue;
        };
        for method in impl_block.methods.iter().filter(|method| {
            !method.is_abstract
                && method.instance.is_some()
                && !method.params.iter().any(|p| contains_open_generic(&p.ty))
                && !contains_open_generic(&method.return_type)
        }) {
            let mut mir_function = lower_function_semantic(
                module,
                method,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(impl_block.target.clone()),
                impl_type_arguments_from_generics(&impl_block.generics),
                host_runtime_callables,
                diagnostics,
            );
            mir_function.display_symbol = format!("{type_name}.{}", method.name);
            functions.push(mir_function);
        }
    }
    for callable in &module.instantiated_callables {
        if callable.function.is_abstract {
            continue;
        }
        let Some(instance) = callable.function.instance
        else {
            continue;
        };
        let mut mir_function = lower_function_semantic(
            module,
            &callable.function,
            aggregate_layouts,
            singleton_accessor_instances,
            callable_returns,
            member_dispatch,
            callable.owner_type.clone(),
            Vec::new(),
            host_runtime_callables,
            diagnostics,
        );
        // 同一声明的不同代入必须有不同的 ABI 标签。
        mir_function.display_symbol = format!("{}#{}", mir_function.display_symbol, instance.index());
        functions.push(mir_function);
    }
    for item in &module.structs {
        for method in item.methods.iter().filter(|method| {
            !method.is_abstract
                && !method.params.iter().any(|p| contains_open_generic(&p.ty))
                && !contains_open_generic(&method.return_type)
        }) {
            let mut mir_function = lower_function_semantic(
                module,
                method,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(ValkyrieType::Named(item.name.clone())),
                impl_type_arguments_from_generics(&item.generics),
                host_runtime_callables,
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", item.name, method.name);
            functions.push(mir_function);
        }
    }
    for trait_def in &module.traits {
        for method in trait_def.default_methods.iter().filter(|method| {
            !method.is_abstract
                && !method.params.iter().any(|p| contains_open_generic(&p.ty))
                && !contains_open_generic(&method.return_type)
        }) {
            let mut mir_function = lower_function_semantic(
                module,
                method,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(ValkyrieType::Named(trait_def.name.clone())),
                impl_type_arguments_from_generics(&trait_def.generics),
                host_runtime_callables,
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", trait_def.name, method.name);
            functions.push(mir_function);
        }
    }
    functions
}
