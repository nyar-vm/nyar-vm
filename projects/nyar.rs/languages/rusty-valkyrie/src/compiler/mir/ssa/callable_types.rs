//! callable 返回类型索引与 impl 方法签名推导。

use super::{MirSumDeclaration, hir_identity::impl_owner_type_name, struct_facts::resolve_self_type_with_owner};
use crate::compiler::{
    hir::{lowering::array_owner_declaration, overload::types::struct_apply_type, symbols::stable_hir_function_symbol},
    types::hir::{HirFunction, HirModule, ValkyrieType},
};
use nyar_types::ItemInstanceId;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct CallableReturnTypes {
    pub(super) by_symbol: BTreeMap<String, ValkyrieType>,
    pub(super) by_instance: BTreeMap<ItemInstanceId, ValkyrieType>,
}

pub(super) fn register_callable_return(index: &mut CallableReturnTypes, symbol: String, function: &HirFunction, return_type: ValkyrieType) {
    index.by_symbol.insert(symbol, return_type.clone());
    if let Some(instance) = function.instance {
        index.by_instance.insert(instance, return_type);
    }
}

pub(super) fn collect_callable_return_types(module: &HirModule) -> CallableReturnTypes {
    let mut index = CallableReturnTypes { by_symbol: BTreeMap::new(), by_instance: BTreeMap::new() };
    collect_callable_return_types_into(module, &mut index);
    index
}

fn collect_callable_return_types_into(module: &HirModule, index: &mut CallableReturnTypes) {
    // 结构体固有方法的 `self` 复原走同一条 owner 折叠，不得用裸 `Named(owner)` 当 self。
    let array_owner = array_owner_declaration(module).ok().flatten();
    for function in &module.functions {
        register_callable_return(index, stable_hir_function_symbol(&module.name, function), function, function.return_type.clone());
    }
    for impl_block in &module.impls {
        let Some(type_name) = impl_owner_type_name(module, &impl_block.target)
        else {
            continue;
        };
        for method in &impl_block.methods {
            register_callable_return(
                index,
                format!("{type_name}.{}", method.name),
                method,
                resolve_self_type_with_owner(&method.return_type, Some(&impl_block.target)),
            );
        }
    }
    for item in &module.structs {
        let owner = struct_apply_type(item, array_owner);
        for method in &item.methods {
            register_callable_return(
                index,
                format!("{}.{}", item.name, method.name),
                method,
                resolve_self_type_with_owner(&method.return_type, Some(&owner)),
            );
        }
    }
    for trait_def in &module.traits {
        for method in &trait_def.default_methods {
            register_callable_return(
                index,
                format!("{}.{}", trait_def.name, method.name),
                method,
                resolve_self_type_with_owner(&method.return_type, None),
            );
        }
    }
    for singleton in &module.singletons {
        let owner = ValkyrieType::Named(singleton.name.clone());
        for method in
            singleton.methods.iter().chain(singleton.constructor.iter().map(Box::as_ref)).chain(singleton.finalizer.iter().map(Box::as_ref))
        {
            register_callable_return(
                index,
                format!("{}.{}", singleton.name, method.name),
                method,
                resolve_self_type_with_owner(&method.return_type, Some(&owner)),
            );
        }
        if let Some(instance) = singleton.accessor_instance {
            index.by_instance.insert(instance, owner);
        }
    }
    for callable in &module.instantiated_callables {
        register_callable_return(
            index,
            stable_hir_function_symbol(&module.name, &callable.function),
            &callable.function,
            resolve_self_type_with_owner(&callable.function.return_type, callable.owner_type.as_ref()),
        );
    }
    for submodule in &module.submodules {
        collect_callable_return_types_into(submodule, index);
    }
}

pub(super) fn impl_type_arguments_from_generics(generics: &[crate::compiler::types::hir::GenericType]) -> Vec<ValkyrieType> {
    generics.iter().map(|generic| ValkyrieType::Generic(generic.clone())).collect()
}

/// `imply Option<T>` 在 HIR 里常把 owner 收成 `Named(Option)`；为 sum 判别与 payload 代入补齐 Apply 实参。
pub(super) fn impl_receiver_type(
    owner: &ValkyrieType,
    impl_type_arguments: &[ValkyrieType],
    sum_types: &[super::MirSumDeclaration],
) -> ValkyrieType {
    if matches!(owner, ValkyrieType::Nominal { arguments, .. } if !arguments.is_empty())
        || matches!(owner, ValkyrieType::Apply(_, args) if !args.is_empty())
    {
        return owner.clone();
    }
    let Some(declaration) = owner.nominal_declaration()
    else {
        // 未绑定 declaration 的短名 owner 不得在 MIR 猜成 sum 身份。
        return owner.clone();
    };
    let type_args: Vec<ValkyrieType> = if !impl_type_arguments.is_empty() {
        impl_type_arguments.to_vec()
    }
    else {
        MirSumDeclaration::find_by_declaration(sum_types, declaration)
            .map(|sum| sum.generics.iter().map(|generic| ValkyrieType::Generic(generic.clone())).collect())
            .unwrap_or_default()
    };
    // owner 已冻结身份的声明补齐实参后仍是同一声明：自举身份随 owner 透传。
    let primitive_identity = owner.primitive_identity().map(Box::from);
    if type_args.is_empty() {
        return ValkyrieType::Nominal { declaration, arguments: Vec::new(), primitive_identity };
    }
    ValkyrieType::Nominal { declaration, arguments: type_args, primitive_identity }
}

/// impl 方法返回类型：只消费 HIR 已冻结的 `Self` 代入与名义实参。
///
/// `Self` 代入直接复用 impl target（它已携带泛型实参）。若返回类型是 owner 声明的
/// 零实参名义形态，而 impl target 该声明带实参，说明源码写了缺参的裸 owner 名：
/// 这是事实缺失，返回 `None` 让调用方结构化诊断。禁止按 owner 泛型数量、方法泛型
/// 位置或 `.last()` 补造返回类型——任意二参数 sum 并不天然具有 `map<T,E> → map<U,E>`
/// 的语义，方法末位泛型也不天然是返回容器的元素类型。
pub(super) fn impl_method_return_type(
    ty: &ValkyrieType,
    impl_owner_type: Option<&ValkyrieType>,
    impl_type_arguments: &[ValkyrieType],
) -> Option<ValkyrieType> {
    let resolved = resolve_self_type_with_owner(ty, impl_owner_type);
    let ValkyrieType::Nominal { declaration, arguments, .. } = &resolved
    else {
        return Some(resolved);
    };
    if !arguments.is_empty() {
        return Some(resolved);
    }
    let Some(owner_arguments) = impl_owner_arguments(impl_owner_type, *declaration)
    else {
        return Some(resolved);
    };
    // impl 头部声明了泛型就必须出现在返回类型里；零实参说明 HIR 未把 binder
    // 代入冻结进签名，这是缺参事实，不得在 MIR 补。
    if owner_arguments.is_empty() {
        return if impl_type_arguments.is_empty() { Some(resolved) } else { None };
    }
    None
}

/// 取 impl target 中给定声明的实参切片；声明不同或形态无法确定时返回 `None`。
fn impl_owner_arguments<'a>(impl_owner_type: Option<&'a ValkyrieType>, declaration: nyar_types::ItemId) -> Option<&'a [ValkyrieType]> {
    match impl_owner_type? {
        ValkyrieType::Nominal { declaration: owner_declaration, arguments, .. } if *owner_declaration == declaration => Some(arguments),
        ValkyrieType::Apply(base, arguments) if base.nominal_declaration() == Some(declaration) => Some(arguments),
        _ => None,
    }
}

/// 接收者/`Self` 参数：只认 Auto/Self → owner 的已冻结 Nominal；禁止按 `Named("Option")` 补身份。
pub(super) fn impl_method_param_type(
    ty: &ValkyrieType,
    param_index: usize,
    impl_owner_type: Option<&ValkyrieType>,
    impl_type_arguments: &[ValkyrieType],
    sum_types: &[super::MirSumDeclaration],
) -> ValkyrieType {
    let _ = param_index;
    match (ty, impl_owner_type) {
        (ValkyrieType::AutoType | ValkyrieType::SelfType, Some(owner)) => impl_receiver_type(owner, impl_type_arguments, sum_types),
        // 已绑定为 owner 声明的无参 Nominal 时补齐 imply 实参。
        (ValkyrieType::Nominal { declaration, arguments, .. }, Some(owner))
            if arguments.is_empty() && owner.nominal_declaration() == Some(*declaration) =>
        {
            impl_receiver_type(owner, impl_type_arguments, sum_types)
        }
        (ty, owner) => resolve_self_type_with_owner(ty, owner),
    }
}
