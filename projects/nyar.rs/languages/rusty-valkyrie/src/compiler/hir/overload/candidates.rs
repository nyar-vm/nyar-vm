//! 候选构造：collect_module_candidates / build_function_candidate 等。

use crate::compiler::types::{
    Identifier, NamePath,
    hir::{
        GenericType, HirDependencySemanticExport, HirEnum, HirExprKind, HirFunction, HirModule, HirParam, HirStringSegment, HirStruct,
        HirTrait, HirVariadicKind, HirVariant, ValkyrieType,
    },
};

use super::{
    diagnostics::{OverloadCandidate, OverloadDomain, OverloadMatchKind},
    types::struct_apply_type,
};
pub(crate) fn declaration_intrinsic(function: &HirFunction) -> Option<nyar_types::IntrinsicId> {
    let attribute =
        function.annotations.iter().find(|attribute| attribute.name.parts().len() == 1 && attribute.name.parts()[0].as_str() == "intrinsic")?;
    let argument = attribute.arguments.first()?;
    let HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::String(literal)) = &argument.value.kind
    else {
        return None;
    };
    let mut rendered = String::new();
    for segment in &literal.segments {
        let HirStringSegment::Text(text) = segment
        else {
            return None;
        };
        rendered.push_str(text);
    }
    let parts = rendered.split('.').collect::<Vec<_>>();
    nyar_types::IntrinsicId::resolve_from_segments(&parts)
}

fn build_function_candidate(module_name: &NamePath, function: &HirFunction, imported_dependency_export: bool) -> OverloadCandidate {
    let symbol = crate::compiler::hir::symbols::stable_hir_function_name_path(module_name, function);
    let mut candidate = OverloadCandidate::new(
        symbol,
        OverloadDomain::Function,
        function.params.iter().map(|param| param.ty.clone()).collect(),
        function.return_type.clone(),
        OverloadMatchKind::Row,
    )
    .with_param_specs(function.params.clone());
    candidate.imported_dependency_export = imported_dependency_export;
    candidate.declaration = function.declaration;
    candidate.instance = function.instance;
    candidate.intrinsic = declaration_intrinsic(function);
    for generic in &function.generics {
        if let Some(binder) = generic.binder {
            candidate = candidate.with_generic_binder(binder);
        }
    }
    candidate.with_where_constraints(function.where_constraints.clone())
}

fn synthetic_field_params(item: &HirStruct) -> Vec<HirParam> {
    item.fields
        .iter()
        .map(|field| HirParam {
            name: crate::compiler::types::hir::HirIdentifier {
                name: field.name.clone(),
                shadow_index: 0,
                span: crate::compiler::types::SourceSpan::new(crate::compiler::types::SourceID::default(), 0, 0),
            },
            ty: field.ty.clone(),
            is_mutable: false,
            binding_kind: crate::compiler::types::hir::HirParameterBindingKind::PositionalOrKeyword,
            default: None,
            variadic: HirVariadicKind::None,
        })
        .collect()
}

fn struct_constructor_path(item: &HirStruct) -> NamePath {
    let mut parts = item.namespace.clone();
    parts.push(item.name.clone());
    NamePath::new(parts)
}

fn build_struct_constructor_candidate(item: &HirStruct) -> OverloadCandidate {
    let param_specs = synthetic_field_params(item);
    let mut candidate = OverloadCandidate::new(
        struct_constructor_path(item),
        OverloadDomain::Constructor,
        item.fields.iter().map(|field| field.ty.clone()).collect(),
        struct_apply_type(item),
        OverloadMatchKind::Row,
    )
    .with_param_specs(param_specs);
    for generic in &item.generics {
        if let Some(binder) = generic.binder {
            candidate = candidate.with_generic_binder(binder);
        }
    }
    candidate.declaration = item.constructor_declaration;
    candidate.instance = item.constructor_instance;
    candidate
}

fn variant_constructor_param_types(variant: &HirVariant) -> Vec<ValkyrieType> {
    variant.fields.iter().map(|field| field.ty.clone()).collect()
}

fn build_variant_constructor_candidate(variant: &HirVariant, enum_def: &HirEnum) -> OverloadCandidate {
    use super::types::enum_apply_type;
    // A variant constructor produces its owning nominal enum/sum. Falling
    // back to `Named(variant.name)` loses the sum identity and forces backend
    // code to guess from the constructor spelling. Keep the explicit result
    // type when present; otherwise the owner is the semantic contract.
    let return_type = variant.result_type.clone().unwrap_or_else(|| enum_apply_type(enum_def));
    let mut candidate = OverloadCandidate::new_method(
        enum_def.name.clone(),
        NamePath::new(vec![variant.name.clone()]),
        OverloadDomain::Constructor,
        variant_constructor_param_types(variant),
        return_type,
        OverloadMatchKind::Row,
    );
    for generic in &enum_def.generics {
        if let Some(binder) = generic.binder {
            candidate = candidate.with_generic_binder(binder);
        }
    }
    candidate.declaration = variant.declaration;
    candidate.instance = variant.instance;
    candidate.sum_owner = enum_def.declaration;
    candidate.sum_variant = variant.declaration;
    candidate
}

fn build_unite_variant_extractor_candidate(enum_def: &HirEnum, variant: &HirVariant) -> OverloadCandidate {
    use super::types::{enum_apply_type, nullable_union_type, variant_payload_type};

    let enum_type = enum_apply_type(enum_def);
    let payload_type = variant_payload_type(variant);
    let return_type = nullable_union_type(payload_type);
    let self_param = HirParam {
        name: crate::compiler::types::hir::HirIdentifier {
            name: Identifier::new("self"),
            shadow_index: 0,
            span: crate::compiler::types::SourceSpan::new(crate::compiler::types::SourceID::default(), 0, 0),
        },
        ty: enum_type.clone(),
        is_mutable: false,
        binding_kind: crate::compiler::types::hir::HirParameterBindingKind::PositionalOrKeyword,
        default: None,
        variadic: HirVariadicKind::None,
    };

    let mut candidate = OverloadCandidate::new_method(
        variant.name.clone(),
        NamePath::new(vec![Identifier::new("extractor")]),
        OverloadDomain::Function,
        vec![enum_type],
        return_type,
        OverloadMatchKind::NominalSubtype { distance: 0 },
    )
    .with_param_specs(vec![self_param]);
    candidate.sum_owner = enum_def.declaration;
    candidate.sum_variant = variant.declaration;
    candidate
}

fn build_method_candidate(function: &HirFunction, owner: Option<Identifier>, trait_owner: Option<&HirTrait>) -> OverloadCandidate {
    let candidate = if let Some(owner) = owner {
        OverloadCandidate::new_method(
            owner,
            NamePath::new(vec![function.name.clone()]),
            OverloadDomain::Function,
            function.params.iter().map(|param| param.ty.clone()).collect(),
            function.return_type.clone(),
            OverloadMatchKind::Row,
        )
    }
    else {
        OverloadCandidate::new(
            trait_owner
                .as_ref()
                .map(|owner| NamePath::new(vec![owner.name.clone(), function.name.clone()]))
                .unwrap_or_else(|| NamePath::new(vec![function.name.clone()])),
            OverloadDomain::Function,
            function.params.iter().map(|param| param.ty.clone()).collect(),
            function.return_type.clone(),
            OverloadMatchKind::Row,
        )
    };
    let mut candidate = candidate.with_param_specs(function.params.clone());
    candidate.declaration = function.declaration;
    candidate.instance = function.instance;
    candidate.intrinsic = declaration_intrinsic(function);
    for generic in &function.generics {
        if let Some(binder) = generic.binder {
            candidate = candidate.with_generic_binder(binder);
        }
    }
    let mut candidate = candidate.with_where_constraints(function.where_constraints.clone());
    candidate.trait_owner = trait_owner.and_then(|owner| owner.declaration);
    if let Some(owner) = trait_owner {
        for generic in &owner.generics {
            if let Some(binder) = generic.binder {
                candidate.generic_binders.insert(binder);
            }
        }
    }
    candidate
}

/// 将方法声明绑定到完整的名义 owner；仅保留 owner 的短名用于 callable identity。
/// `Self` 和 impl 泛型必须在候选进入实参统一前完成替换，否则泛型实例方法会
/// 被降成非泛型 nominal 类型，函数体中的 receiver 也会失去同一份代入事实。
pub(crate) fn specialize_method_candidate(
    mut candidate: OverloadCandidate,
    owner_type: &ValkyrieType,
    owner_generics: &[GenericType],
) -> OverloadCandidate {
    use super::substitution::{owner_type_generic_binders, specialize_method_param_type, substitute_type_self};

    candidate.signature.params = candidate
        .signature
        .params
        .iter()
        .enumerate()
        .map(|(index, ty)| specialize_method_param_type(ty, index, &candidate.param_specs, owner_type))
        .collect();
    candidate.signature.return_type = substitute_type_self(&candidate.signature.return_type, owner_type);
    candidate.param_specs = candidate
        .param_specs
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let mut param = param.clone();
            param.ty = specialize_method_param_type(&param.ty, index, &candidate.param_specs, owner_type);
            param
        })
        .collect();
    for generic in owner_generics {
        if let Some(binder) = generic.binder {
            candidate = candidate.with_generic_binder(binder);
        }
    }
    for generic in owner_type_generic_binders(owner_type) {
        candidate = candidate.with_generic_binder(generic);
    }
    candidate
}

/// 收集模块内全部候选（语言内建候选加已声明候选）；外部测试入口。
#[doc(hidden)]
pub fn collect_module_candidates(module: &HirModule) -> Vec<OverloadCandidate> {
    let mut candidates = language_builtin_candidates();
    candidates.extend(collect_declared_candidates(module));
    candidates
}

fn collect_declared_candidates(module: &HirModule) -> Vec<OverloadCandidate> {
    use super::r#trait::{impl_owner_identifier, impl_owner_identifier_from_export};

    let mut candidates = Vec::new();
    candidates.extend(module.functions.iter().map(|function| build_function_candidate(&module.name, function, false)));
    let local_function_names = module.functions.iter().map(|function| function.name.clone()).collect::<std::collections::BTreeSet<_>>();
    for submodule in &module.submodules {
        candidates.extend(collect_declared_candidates(submodule));
    }
    for item in &module.structs {
        candidates.push(build_struct_constructor_candidate(item));
    }
    for item in &module.structs {
        candidates.extend(item.methods.iter().map(|method| {
            specialize_method_candidate(build_method_candidate(method, Some(item.name.clone()), None), &struct_apply_type(item), &item.generics)
        }));
    }
    for item in &module.singletons {
        let owner_type = ValkyrieType::Named(item.name.clone());
        candidates.extend(
            item.methods
                .iter()
                .map(|method| specialize_method_candidate(build_method_candidate(method, Some(item.name.clone()), None), &owner_type, &[])),
        );
    }
    for item in &module.traits {
        candidates.extend(item.methods.iter().map(|method| build_method_candidate(method, None, Some(item))));
        candidates.extend(item.default_methods.iter().map(|method| build_method_candidate(method, None, Some(item))));
    }
    for item in &module.impls {
        let owner = impl_owner_identifier(module, &item.target);
        candidates.extend(
            item.methods
                .iter()
                .map(|method| specialize_method_candidate(build_method_candidate(method, owner.clone(), None), &item.target, &item.generics)),
        );
    }
    for export in &module.imported_semantic_exports {
        candidates.extend(
            export
                .functions
                .iter()
                .filter(|function| !local_function_names.contains(&function.name))
                .filter(|function| crate::compiler::emitter::backend_contract::interop::function_host_provider_target(function).is_none())
                .map(|function| build_function_candidate(&export.module, function, true)),
        );
        for item in &export.structs {
            candidates.push(build_struct_constructor_candidate(item));
            candidates.extend(item.methods.iter().map(|method| {
                specialize_method_candidate(
                    build_method_candidate(method, Some(item.name.clone()), None),
                    &struct_apply_type(item),
                    &item.generics,
                )
            }));
        }
        for item in &export.enums {
            candidates.extend(item.variants.iter().map(|variant| build_variant_constructor_candidate(variant, item)));
            if item.is_unity {
                candidates.extend(item.variants.iter().map(|variant| build_unite_variant_extractor_candidate(item, variant)));
            }
        }
        for item in &export.traits {
            candidates.extend(item.methods.iter().map(|method| build_method_candidate(method, None, Some(item))));
            candidates.extend(item.default_methods.iter().map(|method| build_method_candidate(method, None, Some(item))));
        }
        for item in &export.impls {
            let owner = impl_owner_identifier(module, &item.target).or_else(|| impl_owner_identifier_from_export(export, &item.target));
            candidates.extend(
                item.methods.iter().map(|method| {
                    specialize_method_candidate(build_method_candidate(method, owner.clone(), None), &item.target, &item.generics)
                }),
            );
        }
    }
    for item in &module.enums {
        candidates.extend(item.variants.iter().map(|variant| build_variant_constructor_candidate(variant, item)));
        if item.is_unity {
            candidates.extend(item.variants.iter().map(|variant| build_unite_variant_extractor_candidate(item, variant)));
        }
    }
    for item in module.imported_nominal_enums() {
        candidates.extend(item.variants.iter().map(|variant| build_variant_constructor_candidate(variant, item)));
        if item.is_unity {
            candidates.extend(item.variants.iter().map(|variant| build_unite_variant_extractor_candidate(item, variant)));
        }
    }
    candidates
}

fn language_builtin_candidates() -> Vec<OverloadCandidate> {
    let element = Identifier::new("T");
    let element_type = ValkyrieType::Named(element.clone());
    let array_type = ValkyrieType::Array(Box::new(element_type.clone()));
    let index_type = ValkyrieType::Named(Identifier::new("usize"));
    let mut get = OverloadCandidate::new(
        NamePath::new(vec![Identifier::new("suffix [ ]")]),
        OverloadDomain::Function,
        vec![array_type.clone(), index_type.clone()],
        element_type.clone(),
        OverloadMatchKind::NominalExact,
    );
    get.intrinsic = Some(nyar_types::IntrinsicId::ArrayGet);
    let mut set = OverloadCandidate::new(
        NamePath::new(vec![Identifier::new("suffix [ ]=")]),
        OverloadDomain::Function,
        vec![array_type.clone(), index_type.clone(), element_type.clone()],
        ValkyrieType::Unit,
        OverloadMatchKind::NominalExact,
    );
    set.intrinsic = Some(nyar_types::IntrinsicId::ArraySet);
    let mut cardinal_get = OverloadCandidate::new(
        NamePath::new(vec![Identifier::new("suffix ⁅ ⁆")]),
        OverloadDomain::Function,
        vec![array_type.clone(), index_type.clone()],
        element_type.clone(),
        OverloadMatchKind::NominalExact,
    );
    cardinal_get.intrinsic = Some(nyar_types::IntrinsicId::ArrayGet);
    let mut cardinal_set = OverloadCandidate::new(
        NamePath::new(vec![Identifier::new("suffix ⁅ ⁆=")]),
        OverloadDomain::Function,
        vec![array_type, index_type, element_type],
        ValkyrieType::Unit,
        OverloadMatchKind::NominalExact,
    );
    cardinal_set.intrinsic = Some(nyar_types::IntrinsicId::ArraySet);
    vec![get, set, cardinal_get, cardinal_set]
}
