//! singleton 与 impl 方法的 MIR lowering。

use super::{
    callable_types::{CallableReturnTypes, impl_type_arguments_from_generics},
    function_lowering::lower_function_semantic,
    hir_identity::impl_owner_type_name,
    model::*,
    value_semantics::AggregateLayoutPlan,
};
use crate::compiler::{
    hir::{HirMaterializationGate, lowering::array_owner_declaration, overload::types::struct_apply_type},
    types::hir::{HirFunction, HirModule, ValkyrieType},
};
use std::collections::BTreeMap;

/// 检查类型里是否还有未闭合的泛型 binder（`ValkyrieType::Generic(generic)` 且 `generic.binder.is_some()`）。
/// impl 方法的原始声明（参数/返回类型里还是 Generic binder）进不了 MIR，让 instantiated_callables 成为唯一来源。
fn contains_open_generic(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Generic(generic) => generic.binder.is_some(),
        ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(contains_open_generic),
        ValkyrieType::Apply(base, arguments) => contains_open_generic(base) || arguments.iter().any(contains_open_generic),
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => contains_open_generic(element),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => items.iter().any(contains_open_generic),
        ValkyrieType::Function(function) => {
            function.params.iter().any(|p| contains_open_generic(p)) || contains_open_generic(&function.return_type)
        }
        ValkyrieType::FixedArray { element, .. } => contains_open_generic(element),
        ValkyrieType::Associated(assoc) => contains_open_generic(&assoc.base) || assoc.type_arguments.iter().any(contains_open_generic),
        _ => false,
    }
}

pub(super) fn lower_singleton_method_functions(
    module: &HirModule,
    gate: Option<&HirMaterializationGate>,
    aggregate_layouts: &mut AggregateLayoutPlan,
    singleton_accessor_instances: &BTreeMap<String, nyar_types::ItemInstanceId>,
    callable_returns: &CallableReturnTypes,
    member_dispatch: &[super::generic_call_instantiation::MirDispatchEntry],
    host_runtime_callables: &BTreeMap<String, nyar_types::ItemInstanceId>,
    lambda_allocator: &std::rc::Rc<super::LambdaInstanceAllocator>,
    diagnostics: &mut Vec<MirDiagnostic>,
) -> Vec<MirFunction> {
    let permits = |function: &HirFunction| gate.is_none_or(|gate| gate.permits_declaration(function.declaration));
    let mut functions = Vec::new();
    for singleton in &module.singletons {
        for method in singleton.methods.iter().filter(|method| {
            !method.is_abstract
                && permits(method)
                && !method.params.iter().any(|p| contains_open_generic(&p.ty))
                && !contains_open_generic(&method.return_type)
        }) {
            let (mut mir_function, nested_functions) = lower_function_semantic(
                module,
                method,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(ValkyrieType::Named(singleton.name.clone())),
                impl_type_arguments_from_generics(&singleton.generics),
                host_runtime_callables,
                std::rc::Rc::clone(lambda_allocator),
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", singleton.name, method.name);
            functions.push(mir_function);
            functions.extend(nested_functions);
        }
        if let Some(constructor) = singleton.constructor.as_deref().filter(|constructor| permits(constructor)) {
            let (mut mir_function, nested_functions) = lower_function_semantic(
                module,
                constructor,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(ValkyrieType::Named(singleton.name.clone())),
                impl_type_arguments_from_generics(&singleton.generics),
                host_runtime_callables,
                std::rc::Rc::clone(lambda_allocator),
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", singleton.name, constructor.name);
            functions.push(mir_function);
            functions.extend(nested_functions);
        }
        if let Some(finalizer) = singleton.finalizer.as_deref().filter(|finalizer| permits(finalizer)) {
            let (mut mir_function, nested_functions) = lower_function_semantic(
                module,
                finalizer,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(ValkyrieType::Named(singleton.name.clone())),
                impl_type_arguments_from_generics(&singleton.generics),
                host_runtime_callables,
                std::rc::Rc::clone(lambda_allocator),
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", singleton.name, finalizer.name);
            functions.push(mir_function);
            functions.extend(nested_functions);
        }
    }
    functions
}

pub(super) fn lower_impl_method_functions(
    module: &HirModule,
    gate: Option<&HirMaterializationGate>,
    aggregate_layouts: &mut AggregateLayoutPlan,
    singleton_accessor_instances: &BTreeMap<String, nyar_types::ItemInstanceId>,
    callable_returns: &CallableReturnTypes,
    member_dispatch: &[super::generic_call_instantiation::MirDispatchEntry],
    host_runtime_callables: &BTreeMap<String, nyar_types::ItemInstanceId>,
    lambda_allocator: &std::rc::Rc<super::LambdaInstanceAllocator>,
    diagnostics: &mut Vec<MirDiagnostic>,
) -> Vec<MirFunction> {
    let permits = |function: &HirFunction| gate.is_none_or(|gate| gate.permits_declaration(function.declaration));
    // 固有方法的 `self` 是声明侧唯一身份的复原：`Array` owner 折成结构形态，其余结构折成 `Nominal`，
    // 不得用裸 `Named(owner)` 充当 self（那是第三种身份）。
    let array_owner = array_owner_declaration(module).ok().flatten();
    let mut functions = Vec::new();
    for impl_block in &module.impls {
        let Some(type_name) = impl_owner_type_name(module, &impl_block.target)
        else {
            continue;
        };
        for method in impl_block.methods.iter().filter(|method| {
            !method.is_abstract
                && method.instance.is_some()
                && permits(method)
                && !method.params.iter().any(|p| contains_open_generic(&p.ty))
                && !contains_open_generic(&method.return_type)
        }) {
            let (mut mir_function, nested_functions) = lower_function_semantic(
                module,
                method,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(impl_block.target.clone()),
                impl_type_arguments_from_generics(&impl_block.generics),
                host_runtime_callables,
                std::rc::Rc::clone(lambda_allocator),
                diagnostics,
            );
            mir_function.display_symbol = format!("{type_name}.{}", method.name);
            functions.push(mir_function);
            functions.extend(nested_functions);
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
        // 铸造副本按实例身份门禁：witness 消解产生的 impl 成员实例声明不在可达
        // 声明集内，其实例身份由可达调用点直接引用，必须按实例放行。
        if gate.is_some_and(|gate| !gate.permits_instance(Some(instance))) {
            continue;
        }
        let (mut mir_function, nested_functions) = lower_function_semantic(
            module,
            &callable.function,
            aggregate_layouts,
            singleton_accessor_instances,
            callable_returns,
            member_dispatch,
            callable.owner_type.clone(),
            Vec::new(),
            host_runtime_callables,
            std::rc::Rc::clone(lambda_allocator),
            diagnostics,
        );
        // 同一声明的不同代入必须有不同的 ABI 标签。
        mir_function.display_symbol = format!("{}#{}", mir_function.display_symbol, instance.index());
        functions.push(mir_function);
        functions.extend(nested_functions);
    }
    // 类/结构体声明体的方法已在 HIR 边界结构化拒绝，`HirStruct.methods` 恒为空。
    // 断言守住“不得再给类体方法接一条 MIR 发射路径”。
    debug_assert!(module.structs.iter().all(|item| item.methods.is_empty()), "HirStruct.methods 必须为空");
    for item in &module.structs {
        for method in item.methods.iter().filter(|method| {
            !method.is_abstract
                && permits(method)
                && !method.params.iter().any(|p| contains_open_generic(&p.ty))
                && !contains_open_generic(&method.return_type)
        }) {
            let (mut mir_function, nested_functions) = lower_function_semantic(
                module,
                method,
                aggregate_layouts,
                singleton_accessor_instances,
                callable_returns,
                member_dispatch,
                Some(struct_apply_type(item, array_owner)),
                impl_type_arguments_from_generics(&item.generics),
                host_runtime_callables,
                std::rc::Rc::clone(lambda_allocator),
                diagnostics,
            );
            mir_function.display_symbol = format!("{}.{}", item.name, method.name);
            functions.push(mir_function);
            functions.extend(nested_functions);
        }
    }
    // trait 默认方法没有可独立降低的实体：其 `Self` 是开放的，只有 witness 消解后
    // 按具体 self 类型铸造的实例（`module.instantiated_callables`）才有确定签名。
    // 在此按 trait 名降一份「默认实现」会产出 `Self = Named(trait)` 的伪 callee。
    functions
}
