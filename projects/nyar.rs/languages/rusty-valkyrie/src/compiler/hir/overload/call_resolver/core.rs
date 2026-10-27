//! 调用解析核心：try_resolve_call / match_call_candidate 等。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::{
        call_binding::bind_call_arguments,
        construct_binding::bind_construct_fields,
        try_propagate::{
            is_option_apply_type, is_result_apply_type, nullable_payload_type, option_apply_payload_type, result_variant_payload_type,
        },
        type_relation::{ParameterMatchResult, TypeRelationContext},
    },
    mir::AggregateFieldIndex,
    semantic::operator_members,
    types::{
        Identifier, NamePath,
        hir::{
            HirCallArgument, HirCallableDomain, HirEnum, HirExpr, HirExprKind, HirFunction, HirLiteral, HirParam, HirPattern, HirResolvedCall,
            HirStringSegment, HirTrait, HirTraitBound, HirVariant, HirWhereConstraint, ValkyrieType,
        },
    },
};

use crate::compiler::hir::overload::{
    diagnostics::{OverloadCandidate, OverloadDomain, OverloadMatchKind, OverloadResolutionError},
    resolve_call::pattern_extractor_receiver_type,
    substitution::{
        extract_singleton_type_name, instantiate_trait_bound_candidate, instantiate_trait_method_candidate, substitute_self_type,
        substitute_type_parameters, substitute_type_self, substitute_type_vars, unify_call_type_binders,
    },
    symbol_match::{
        aggregate_constructor_matches_name, candidate_extractor_path_matches, candidate_owned_method_matches, candidate_trait_method_matches,
        candidate_variant_constructor_matches, symbol_matches_imported_dependency_export, symbol_matches_language_builtin,
        symbol_matches_unqualified_free_call,
    },
    r#trait::impl_nominal_type_name,
    types::{
        accepts_array_shape, compute_call_match_kind, extract_callable_name, infer_expr_type, infer_scrutinee_type, is_empty_array_literal,
    },
};

use super::{
    constructors::{
        try_resolve_constructor_call, try_resolve_instance_method, try_resolve_qualified_free_function, try_resolve_singleton_method,
        try_resolve_type_static_method,
    },
    patterns::enrich_sum_variant_operation_facts,
};

/// 候选是否以 `self` 为首个形参。
pub(crate) fn candidate_has_receiver_parameter(candidate: &OverloadCandidate) -> bool {
    candidate.param_specs.first().is_some_and(|parameter| parameter.name.name.as_str() == "self")
}

pub(crate) fn overload_symbol_path(candidate: &OverloadCandidate) -> NamePath {
    if let Some(owner) = &candidate.owner {
        let parts = candidate.symbol.parts();
        if parts.first().is_some_and(|head| head == owner) {
            return candidate.symbol.clone();
        }
        let mut qualified = vec![owner.clone()];
        qualified.extend(parts.iter().cloned());
        NamePath::new(qualified)
    }
    else {
        candidate.symbol.clone()
    }
}

pub(crate) fn is_unresolved_variant_reference(expr: &HirExpr, locals: &BTreeMap<String, ValkyrieType>) -> bool {
    match &expr.kind {
        HirExprKind::Variable(identifier) => !locals.contains_key(identifier.name.as_str()),
        HirExprKind::Path(path) => path.parts().len() == 1 && !locals.contains_key(path.parts()[0].as_str()),
        _ => false,
    }
}

pub(crate) fn peel_explicit_generic_arguments(callee: &HirExpr) -> (Vec<ValkyrieType>, &HirExpr) {
    match &callee.kind {
        HirExprKind::GenericApply { callee, arguments } => (arguments.clone(), callee.as_ref()),
        _ => (Vec::new(), callee),
    }
}

/// 语言 builtin：以候选符号的 IntrinsicId 为准。
pub(crate) fn match_intrinsic_builtin_candidate(
    candidate: &OverloadCandidate,
    args: &[HirCallArgument],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<OverloadCandidate> {
    let intrinsic = candidate.intrinsic?;
    let mk = |signature: crate::compiler::hir::overload::diagnostics::OverloadSignature| OverloadCandidate {
        declaration: candidate.declaration,
        instance: candidate.instance,
        sum_owner: None,
        sum_variant: None,
        intrinsic: candidate.intrinsic,
        symbol: candidate.symbol.clone(),
        owner: None,
        trait_owner: candidate.trait_owner.clone(),
        domain: OverloadDomain::Function,
        signature,
        match_kind: OverloadMatchKind::NominalExact,
        param_specs: Vec::new(),
        generic_binders: candidate.generic_binders.clone(),
        where_constraints: candidate.where_constraints.clone(),
        imported_dependency_export: candidate.imported_dependency_export,
    };
    fn infer(
        expr: &HirExpr,
        locals: &BTreeMap<String, ValkyrieType>,
        struct_fields: &AggregateFieldIndex,
        singleton_names: &std::collections::BTreeSet<Identifier>,
    ) -> Option<ValkyrieType> {
        crate::compiler::hir::overload::types::infer_scrutinee_type(expr, locals, struct_fields, singleton_names)
    }

    match intrinsic {
        nyar_types::IntrinsicId::ArrayPush => {
            if args.len() != 2 {
                return None;
            }
            let array_ty = infer(&args[0].value, locals, struct_fields, singleton_names)?;
            let ValkyrieType::Array(element) = &array_ty
            else {
                return None;
            };
            let element = element.as_ref().clone();
            let value_ty = infer(&args[1].value, locals, struct_fields, singleton_names)?;
            if matches!(value_ty, ValkyrieType::AutoType | ValkyrieType::r#SelfType)
                || matches!(type_relations.match_parameter(&value_ty, &element), ParameterMatchResult::NoMatch { .. })
            {
                return None;
            }
            Some(mk(crate::compiler::hir::overload::diagnostics::OverloadSignature {
                params: vec![array_ty.clone(), element],
                return_type: array_ty,
            }))
        }
        nyar_types::IntrinsicId::ArrayLen => {
            if args.len() != 1 {
                return None;
            }
            let array_ty = infer(&args[0].value, locals, struct_fields, singleton_names)?;
            if !matches!(array_ty, ValkyrieType::Array(_) | ValkyrieType::FixedArray { .. }) {
                return None;
            }
            let return_type =
                type_relations.nominal_type_for_name(&Identifier::new("usize")).unwrap_or_else(|| candidate.signature.return_type.clone());
            Some(mk(crate::compiler::hir::overload::diagnostics::OverloadSignature { params: vec![array_ty], return_type }))
        }
        nyar_types::IntrinsicId::RefDeref => {
            if args.len() != 1 {
                return None;
            }
            let operand_ty = infer(&args[0].value, locals, struct_fields, singleton_names)?;
            let return_type = nullable_payload_type(&operand_ty).unwrap_or_else(|| candidate.signature.return_type.clone());
            Some(mk(crate::compiler::hir::overload::diagnostics::OverloadSignature { params: vec![operand_ty], return_type }))
        }
        nyar_types::IntrinsicId::IsNull => {
            if args.len() != 1 {
                return None;
            }
            let operand_ty = infer(&args[0].value, locals, struct_fields, singleton_names)?;
            if !crate::compiler::hir::is_nullable_type(&operand_ty) {
                return None;
            }
            Some(mk(crate::compiler::hir::overload::diagnostics::OverloadSignature {
                params: vec![operand_ty],
                return_type: ValkyrieType::Boolean,
            }))
        }
        nyar_types::IntrinsicId::UnwrapNull => {
            if args.len() != 1 {
                return None;
            }
            let operand_ty = infer(&args[0].value, locals, struct_fields, singleton_names)?;
            let Some(payload) = nullable_payload_type(&operand_ty)
            else {
                return None;
            };
            Some(mk(crate::compiler::hir::overload::diagnostics::OverloadSignature { params: vec![operand_ty], return_type: payload }))
        }
        nyar_types::IntrinsicId::ArrayGet => {
            if args.len() != 2 {
                return None;
            }
            let array_ty = infer(&args[0].value, locals, struct_fields, singleton_names)?;
            let element = match &array_ty {
                ValkyrieType::Array(e) => e.as_ref().clone(),
                ValkyrieType::FixedArray { element, .. } => element.as_ref().clone(),
                _ => return None,
            };
            let index_ty = infer(&args[1].value, locals, struct_fields, singleton_names)?;
            if !type_relations.is_numeric_type(&index_ty) {
                return None;
            }
            // ArrayGet intrinsic 语义上始终返回裸元素类型 T。
            // Option 由调用方通过 Some() 包裹（如 Array.get: `return Some(__array_get(...))`）。
            // 禁止 intrinsic 签名自身携带 Option，避免 Some() 包裹时产生 Option<Option<T>>。
            let return_type = element.clone();
            Some(mk(crate::compiler::hir::overload::diagnostics::OverloadSignature { params: vec![array_ty, index_ty], return_type }))
        }
        nyar_types::IntrinsicId::ArraySet => {
            if args.len() != 3 {
                return None;
            }
            let array_ty = infer(&args[0].value, locals, struct_fields, singleton_names)?;
            let element = match &array_ty {
                ValkyrieType::Array(e) => e.as_ref().clone(),
                ValkyrieType::FixedArray { element, .. } => element.as_ref().clone(),
                _ => return None,
            };
            let index_ty = infer(&args[1].value, locals, struct_fields, singleton_names)?;
            if !type_relations.is_numeric_type(&index_ty) {
                return None;
            }
            let value_ty = infer(&args[2].value, locals, struct_fields, singleton_names)?;
            if matches!(value_ty, ValkyrieType::AutoType | ValkyrieType::r#SelfType)
                || matches!(type_relations.match_parameter(&value_ty, &element), ParameterMatchResult::NoMatch { .. })
            {
                return None;
            }
            Some(mk(crate::compiler::hir::overload::diagnostics::OverloadSignature {
                params: vec![array_ty, index_ty, element],
                return_type: ValkyrieType::Unit,
            }))
        }
        _ => None,
    }
}

/// 以实参类型判定单个候选是否匹配；外部测试入口。
#[doc(hidden)]
pub fn match_call_candidate(
    candidate: &OverloadCandidate,
    args: &[HirCallArgument],
    explicit_generic_arguments: &[ValkyrieType],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<OverloadCandidate> {
    if candidate.owner.is_none()
        && candidate.trait_owner.is_some()
        && matches!(candidate.signature.params.first(), Some(ValkyrieType::AutoType | ValkyrieType::r#SelfType))
    {
        return None;
    }
    if let Some(matched) = match_intrinsic_builtin_candidate(candidate, args, type_relations, locals, struct_fields, singleton_names) {
        return Some(matched);
    }
    let bound_exprs: Vec<HirExpr> = if candidate.param_specs.is_empty() {
        args.iter().map(|arg| arg.value.clone()).collect()
    }
    else {
        bind_call_arguments(&candidate.param_specs, args).ok()?
    };
    let actual_types = bound_exprs.iter().map(|expr| infer_scrutinee_type(expr, locals, struct_fields, singleton_names)).collect::<Vec<_>>();
    let owner_type = candidate.owner.as_ref().and_then(|owner| {
        actual_types
            .first()
            .and_then(|actual| actual.as_ref())
            .filter(|actual| impl_nominal_type_name(actual) == Some(owner.as_str()))
            .cloned()
            .or_else(|| type_relations.nominal_type_for_name(owner))
    });
    let expected_params = candidate
        .signature
        .params
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let is_self_slot = index == 0
                && candidate.owner.is_some()
                && candidate.param_specs.first().is_some_and(|param| param.name.name.as_str() == "self");
            if is_self_slot && matches!(ty, ValkyrieType::AutoType | ValkyrieType::r#SelfType) {
                if let Some(owner_type) = &owner_type {
                    return owner_type.clone();
                }
            }
            match &owner_type {
                Some(owner_type) => substitute_type_self(ty, owner_type),
                None => substitute_self_type(ty, candidate.owner.as_ref()),
            }
        })
        .collect::<Vec<_>>();
    let mut substitutions = BTreeMap::new();
    if !explicit_generic_arguments.is_empty() {
        let binders = candidate.generic_binders.iter().cloned().collect::<Vec<_>>();
        if explicit_generic_arguments.len() != binders.len() {
            return None;
        }
        for (binder, argument) in binders.iter().zip(explicit_generic_arguments.iter()) {
            substitutions.insert(binder.clone(), argument.clone());
        }
    }
    let mut matched_actual_types = Vec::with_capacity(bound_exprs.len());
    for (expr, (expected, actual)) in bound_exprs.iter().zip(expected_params.iter().zip(actual_types.iter())) {
        let contextual_expected = substitute_type_vars(expected, &substitutions);
        let actual = actual.as_ref().map(|actual| {
            crate::compiler::hir::overload::substitution::specialize_literal_expr_type(expr, actual, &contextual_expected, type_relations)
        });
        match actual.as_ref() {
            Some(actual) => unify_call_type_binders(expected, actual, &candidate.generic_binders, &mut substitutions)?,
            None if is_empty_array_literal(expr) && accepts_array_shape(&contextual_expected) => {}
            None => return None,
        }
        matched_actual_types.push(actual);
    }
    let expected_params = expected_params.iter().map(|ty| substitute_type_vars(ty, &substitutions)).collect::<Vec<_>>();
    if !type_relations.satisfies_where_constraints(&candidate.where_constraints, &substitutions) {
        return None;
    }
    let matched_actual_types = matched_actual_types
        .iter()
        .zip(expected_params.iter())
        .map(|(actual, expected)| actual.clone().unwrap_or_else(|| expected.clone()))
        .collect::<Vec<_>>();
    let match_kind = compute_call_match_kind(type_relations, &matched_actual_types, &expected_params)?;
    Some(OverloadCandidate {
        declaration: candidate.declaration,
        instance: candidate.instance,
        sum_owner: candidate.sum_owner,
        sum_variant: candidate.sum_variant,
        intrinsic: candidate.intrinsic,
        symbol: candidate.symbol.clone(),
        owner: candidate.owner.clone(),
        trait_owner: candidate.trait_owner.clone(),
        domain: candidate.domain.clone(),
        signature: crate::compiler::hir::overload::diagnostics::OverloadSignature {
            params: expected_params,
            return_type: substitute_type_vars(&candidate.signature.return_type, &substitutions),
        },
        match_kind,
        param_specs: candidate.param_specs.clone(),
        generic_binders: candidate.generic_binders.clone(),
        where_constraints: candidate.where_constraints.clone(),
        imported_dependency_export: candidate.imported_dependency_export,
    })
}

pub(crate) fn try_resolve_call(
    callee: &HirExpr,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) -> Option<HirResolvedCall> {
    let (explicit_generic_arguments, callee) = peel_explicit_generic_arguments(callee);
    if let Some(local_name) = match &callee.kind {
        HirExprKind::Variable(identifier) => Some(identifier.name.as_str()),
        HirExprKind::Path(path) if path.parts().len() == 1 => Some(path.parts()[0].as_str()),
        _ => None,
    } {
        if let Some(ValkyrieType::Function(func)) = locals.get(local_name) {
            return Some(HirResolvedCall {
                declaration: None,
                instance: None,
                sum_owner: None,
                sum_variant: None,
                intrinsic: None,
                symbol: NamePath::new(vec![Identifier::new(local_name)]),
                domain: HirCallableDomain::Function,
                return_type: func.return_type.clone(),
                parameter_types: func.params.clone(),
                has_receiver: false,
                extractor_payload_type: None,
            });
        }
        if locals.contains_key(local_name) {
            return None;
        }
    }
    if let HirExprKind::Path(path) = &callee.kind {
        if path.parts().len() == 2 {
            let owner = &path.parts()[0];
            let method_name = &path.parts()[1];
            let subjects = type_relations.callable_constraints().iter().filter(|constraint| {
                matches!(&constraint.target, ValkyrieType::Generic(generic) if generic.binder.is_some() && generic.name == *owner)
            }).collect::<Vec<_>>();
            if let [subject] = subjects.as_slice() {
                let mut matches = Vec::new();
                for bound in &subject.bounds {
                    for candidate in candidates
                        .iter()
                        .filter(|candidate| !candidate_has_receiver_parameter(candidate))
                        .filter(|candidate| candidate_trait_method_matches(candidate, method_name))
                        .filter(|candidate| bound.declaration.is_some() && candidate.trait_owner == bound.declaration)
                    {
                        let candidate = instantiate_trait_bound_candidate(candidate, &subject.target, bound)?;
                        if let Some(matched) = match_call_candidate(
                            &candidate,
                            args,
                            &explicit_generic_arguments,
                            type_relations,
                            locals,
                            struct_fields,
                            singleton_names,
                        ) {
                            matches.push(matched);
                        }
                    }
                }
                let matched = crate::compiler::hir::overload::diagnostics::resolve_overload(&matches).ok()?;
                return Some(HirResolvedCall {
                    declaration: matched.declaration,
                    instance: matched.instance,
                    sum_owner: matched.sum_owner,
                    sum_variant: matched.sum_variant,
                    intrinsic: None,
                    symbol: matched.symbol,
                    domain: HirCallableDomain::Function,
                    return_type: matched.signature.return_type,
                    parameter_types: matched.signature.params,
                    has_receiver: false,
                    extractor_payload_type: None,
                });
            }
            if singleton_names.contains(owner) {
                return try_resolve_singleton_method(
                    owner,
                    method_name,
                    args,
                    candidates,
                    type_relations,
                    locals,
                    struct_fields,
                    singleton_names,
                    true,
                    false,
                );
            }
        }
    }

    if let HirExprKind::FieldAccess { object, field, .. } = &callee.kind {
        return try_resolve_instance_method(object, field, args, candidates, type_relations, locals, struct_fields, singleton_names);
    }
    if let HirExprKind::Path(path) = &callee.kind {
        if path.parts().len() >= 2 && locals.contains_key(path.parts()[0].as_str()) {
            let method_name = path.parts().last().expect("path has parts");
            let span = callee.span.clone();
            let mut receiver = HirExpr {
                kind: HirExprKind::Variable(crate::compiler::types::hir::HirIdentifier {
                    name: path.parts()[0].clone(),
                    shadow_index: 0,
                    span: span.clone(),
                }),
                span: span.clone(),
            };
            for field in &path.parts()[1..path.parts().len() - 1] {
                receiver = HirExpr {
                    kind: HirExprKind::FieldAccess { object: Box::new(receiver), field: field.clone(), field_id: None },
                    span: span.clone(),
                };
            }
            return try_resolve_instance_method(
                &receiver,
                method_name,
                args,
                candidates,
                type_relations,
                locals,
                struct_fields,
                singleton_names,
            );
        }
    }
    if let HirExprKind::Path(path) = &callee.kind {
        if path.parts().len() >= 2 {
            let head = path.parts().first().map(|p| p.as_str()).unwrap_or("");
            if head != "builtin" {
                let type_owner = &path.parts()[path.parts().len() - 2];
                let method_name = path.parts().last().expect("qualified call path must have a final segment");
                if singleton_names.contains(type_owner) {
                    return try_resolve_singleton_method(
                        type_owner,
                        method_name,
                        args,
                        candidates,
                        type_relations,
                        locals,
                        struct_fields,
                        singleton_names,
                        true,
                        false,
                    );
                }
                if candidates
                    .iter()
                    .any(|candidate| candidate.owner.as_ref() == Some(type_owner) && candidate_owned_method_matches(candidate, method_name))
                {
                    return try_resolve_type_static_method(
                        type_owner,
                        method_name,
                        args,
                        candidates,
                        type_relations,
                        locals,
                        struct_fields,
                        singleton_names,
                        contextual_type,
                    );
                }
                return try_resolve_qualified_free_function(path, args, candidates, type_relations, locals, struct_fields, singleton_names);
            }
        }
    }

    let callee_name = extract_callable_name(callee)?;
    if let Some(owner) = args.first().and_then(|arg| extract_singleton_type_name(&arg.value, locals, singleton_names)) {
        if let Some(resolved) = try_resolve_singleton_method(
            &owner,
            &callee_name,
            args,
            candidates,
            type_relations,
            locals,
            struct_fields,
            singleton_names,
            true,
            true,
        ) {
            return Some(resolved);
        }
    }

    if let Some(resolved) =
        try_resolve_constructor_call(callee, args, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type)
    {
        return Some(resolved);
    }

    if matches!(&callee.kind, HirExprKind::Path(path) if path.parts().len() == 1)
        && operator_members::is_operator_member_name(callee_name.as_str())
        && let Some(first) = args.first()
    {
        let method_args = if callee_name.as_str().starts_with("prefix ") && args.len() == 1 {
            &[]
        }
        else if args.len() > 1 {
            &args[1..]
        }
        else {
            &[]
        };
        if let Some(resolved) = try_resolve_instance_method(
            &first.value,
            &callee_name,
            method_args,
            candidates,
            type_relations,
            locals,
            struct_fields,
            singleton_names,
        ) {
            return Some(resolved);
        }
        // Fallback：primitive numeric / boolean 类型没有显式 imply 时，
        // 注入 built-in operator intrinsic synthetic callable。
        let fb = crate::compiler::hir::overload::call_resolver::primitive_operator::try_resolve_primitive_numeric_operator(
            &first.value,
            callee_name.as_str(),
            method_args,
            locals,
            struct_fields,
            singleton_names,
        );
        if let Some(resolved) = fb {
            return Some(resolved);
        }
    }

    let enclosing_module = type_relations.enclosing_module();
    let filtered = candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Function && candidate.owner.is_none())
        .filter(|candidate| {
            symbol_matches_unqualified_free_call(&candidate.symbol, &callee_name, enclosing_module)
                || symbol_matches_imported_dependency_export(candidate, &callee_name)
                || symbol_matches_language_builtin(candidate, &callee_name)
        })
        .filter_map(|candidate| {
            match_call_candidate(candidate, args, &explicit_generic_arguments, type_relations, locals, struct_fields, singleton_names)
        })
        .collect::<Vec<_>>();
    let resolved = crate::compiler::hir::overload::diagnostics::resolve_overload(&filtered).ok()?;
    let has_receiver = candidate_has_receiver_parameter(&resolved);
    let symbol = overload_symbol_path(&resolved);
    let return_type = resolved.signature.return_type;
    let mut call = HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        intrinsic: resolved.intrinsic,
        symbol,
        domain: HirCallableDomain::Function,
        return_type,
        parameter_types: resolved.signature.params,
        has_receiver,
        extractor_payload_type: None,
    };
    if matches!(callee_name.as_str(), "unwrap" | "is_some") {
        let receiver_type = args.first().and_then(|arg| infer_scrutinee_type(&arg.value, locals, struct_fields, singleton_names));
        enrich_sum_variant_operation_facts(&mut call, receiver_type.as_ref(), candidates);
    }
    Some(call)
}
