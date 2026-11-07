//! 调用解析核心：try_resolve_call / match_call_candidate 等。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::{
        call_binding::bind_call_arguments,
        construct_binding::bind_construct_fields,
        overload::{
            diagnostics::{OverloadCandidate, OverloadDomain, OverloadResolutionError},
            resolve_call::{pattern_extractor_receiver_type, record_failure},
            substitution::{
                extract_singleton_type_name, instantiate_trait_bound_candidate, instantiate_trait_method_candidate, substitute_self_type,
                substitute_type_parameters, substitute_type_self, substitute_type_vars, unify_call_type_binders,
            },
            symbol_match::{
                aggregate_constructor_matches_name, candidate_extractor_path_matches, candidate_owned_method_matches,
                candidate_trait_method_matches, candidate_variant_constructor_matches, symbol_matches_imported_dependency_export,
                symbol_matches_unqualified_free_call,
            },
            r#trait::impl_nominal_type_name,
            types::{
                accepts_array_shape, compute_call_match_kind, extract_callable_name, infer_expr_type, infer_scrutinee_type,
                is_empty_array_literal, lookup_struct_field_type,
            },
        },
        try_propagate::{is_option_apply_type, is_result_apply_type, option_apply_payload_type, result_variant_payload_type},
        type_relation::TypeRelationContext,
    },
    mir::AggregateFieldIndex,
    semantic::operator_members,
    types::{
        Identifier, NamePath, SourceSpan,
        hir::{
            HirCallArgument, HirCallResolutionFailure, HirCallResolutionFailureKind, HirCallableDomain, HirEnum, HirExpr, HirExprKind,
            HirFunction, HirParam, HirPattern, HirResolvedCall, HirTrait, HirTraitBound, HirTraitDispatch, HirVariant, HirWhereConstraint,
            ValkyrieType,
        },
    },
};

use super::{
    constructors::{
        try_resolve_constructor_call, try_resolve_instance_method, try_resolve_instance_method_with_context,
        try_resolve_qualified_free_function, try_resolve_singleton_method,
        try_resolve_type_static_method,
    },
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

/// 函数值调用的统一合同：无声明、无实例、无接收者槽，签名就是函数值自身的类型。
/// 局部变量的函数值调用与结构字段的函数值调用共用这一份结算结果。
fn function_value_call(symbol: NamePath, func: &crate::compiler::types::hir::FunctionType) -> HirResolvedCall {
    HirResolvedCall {
        declaration: None,
        instance: None,
        sum_owner: None,
        sum_variant: None,
        trait_dispatch: None,
        symbol,
        domain: HirCallableDomain::Function,
        return_type: func.return_type.clone(),
        parameter_types: func.params.clone(),
        has_receiver: false,
        extractor_payload_type: None,
        // 函数值没有声明级 where 义务，不存在需要携带的 binder 解。
        associated_binder_solutions: BTreeMap::new(),
    }
}

/// 函数型结构字段调用（`receiver.field(args)`）：字段表冻结的字段类型本身是函数值时，
/// 按函数值调用结算。字段不是函数值即返回 `None`，由调用方保持结构化「无候选」。
fn function_typed_field_call(
    object: &HirExpr,
    field: &Identifier,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<HirResolvedCall> {
    let object_type = infer_expr_type(object, locals)
        .or_else(|| infer_scrutinee_type(object, locals, struct_fields, singleton_names))?;
    let ValkyrieType::Function(func) = lookup_struct_field_type(&object_type, field, struct_fields)?
    else {
        return None;
    };
    Some(function_value_call(NamePath::new(vec![field.clone()]), &func))
}

/// 上下文相关的 lambda 实参类型：仅当形参槽位期望函数类型、且 lambda 自身的
/// 形参与返回类型都已标注时，才把该签名作为实参类型参与匹配。
fn infer_contextual_lambda_type(expr: &HirExpr, expected: &ValkyrieType) -> Option<ValkyrieType> {
    let ValkyrieType::Function(_) = expected
    else {
        return None;
    };
    let HirExprKind::Lambda { params, return_type, .. } = &expr.kind
    else {
        return None;
    };
    if matches!(return_type, ValkyrieType::AutoType) || params.iter().any(|param| matches!(param.ty, ValkyrieType::AutoType)) {
        return None;
    }
    Some(ValkyrieType::Function(Box::new(crate::compiler::types::hir::FunctionType {
        params: params.iter().map(|param| param.ty.clone()).collect(),
        return_type: return_type.clone(),
    })))
}

/// 上下文相关的裸变体名实参类型（`None` / `Word` / `Mark`）。
///
/// 无载荷变体名本身没有自足类型：`infer_scrutinee_type` 对它返回 `None`，实参循环
/// 便在结构化淘汰里丢掉候选。只有形参槽位已冻结为具体 `unite` 声明、且该声明含同名
/// 无载荷变体时，才把槽位类型当作实参类型。局部绑定、单例名、带载荷变体以及仍是
/// 开放 binder 的槽位一律不参与，保持原有「未知即淘汰」。
fn infer_contextual_variant_type(
    expr: &HirExpr,
    expected: &ValkyrieType,
    locals: &BTreeMap<String, ValkyrieType>,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    type_relations: &TypeRelationContext,
) -> Option<ValkyrieType> {
    let name = match &expr.kind {
        HirExprKind::Variable(identifier) => &identifier.name,
        HirExprKind::Path(path) if path.parts().len() == 1 => &path.parts()[0],
        _ => return None,
    };
    if locals.contains_key(name.as_str()) || singleton_names.contains(name) {
        return None;
    }
    type_relations.is_unit_variant_of(expected, name).then(|| expected.clone())
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
    // trait 方法槽没有实现证据：首参是 `Self` 占位时，只有 trait 声明体内部
    // （`self` 已冻结为 `Self`）才允许参与匹配；其它接收者一律拒绝，禁止按名字兜底。
    if candidate.owner.is_none()
        && candidate.trait_owner.is_some()
        && matches!(candidate.signature.params.first(), Some(ValkyrieType::AutoType | ValkyrieType::r#SelfType))
        && !type_relations.is_trait_self_scope(candidate.trait_owner)
    {
        return None;
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
            .filter(|actual| match actual {
                // receiver 已冻结为 `Nominal{ItemId}` 时没有表面名，必须按 declaration
                // 身份核对 owner，才能拿到带实参的 owner 形态（`Bag<u8>` 而不是 `Bag`）。
                ValkyrieType::Nominal { declaration, .. } => type_relations.owner_name_matches_declaration(owner, *declaration),
                other => impl_nominal_type_name(other) == Some(owner.as_str()),
            })
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
    // 实参绑定结果与签名槽位必须等长：带 `param_specs` 的候选由 `bind_call_arguments`
    // 补齐默认 / rest 后恒等长；无 `param_specs` 的候选（unite 变体构造）直接取实参。
    // 不设这道守卫时，后面的 `zip` 会静默截断，让零参候选对多参调用「空匹配」，
    // 与真正匹配的候选并列后判 Ambiguous，最终退化成「无一候选」。
    if bound_exprs.len() != expected_params.len() {
        return None;
    }
    let mut substitutions = BTreeMap::new();
    // owner binder 由声明冻结期写定：self 槽已携带带 binder 的 owner 形态，
    // 与 receiver 实参的统一即可钉住实参，调用点不再按 receiver 回填。
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
        let actual = actual
            .as_ref()
            .map(|actual| {
                crate::compiler::hir::overload::substitution::specialize_literal_expr_type(expr, actual, &contextual_expected, type_relations)
            })
            .or_else(|| infer_contextual_lambda_type(expr, &contextual_expected))
            .or_else(|| infer_contextual_variant_type(expr, &contextual_expected, locals, singleton_names, type_relations));
        match actual.as_ref() {
            Some(actual) => {
                if unify_call_type_binders(expected, actual, &candidate.generic_binders, &mut substitutions, type_relations).is_none() {
                    return None;
                }
            }
            None if is_empty_array_literal(expr) && accepts_array_shape(&contextual_expected) => {}
            None => return None,
        }
        matched_actual_types.push(actual);
    }
    // `where I: Iterator<Item = T>` 里 `T` 可能只出现在关联等式右侧：接收者已锁定 `I`
    // 后，用 impl witness 的关联类型补出 `T`，再按常规路径核对 where 证据。
    type_relations.infer_associated_binders(&candidate.where_constraints, &mut substitutions, &candidate.generic_binders);
    // 形参槽位可能是关联投影（`L::Settings`）：binder 由实参钉住后，用候选自身
    // where 约束的 trait 实现见证把投影落成具体类型，再参与常规匹配。
    let expected_params = expected_params
        .iter()
        .map(|ty| type_relations.resolve_associated_projections(ty, &candidate.where_constraints, &substitutions))
        .collect::<Vec<_>>();
    if !type_relations.satisfies_where_constraints(&candidate.where_constraints, &substitutions) {
        return None;
    }
    let matched_actual_types = matched_actual_types
        .iter()
        .zip(expected_params.iter())
        .map(|(actual, expected)| actual.clone().unwrap_or_else(|| expected.clone()))
        .collect::<Vec<_>>();
    let match_kind = compute_call_match_kind(type_relations, &matched_actual_types, &expected_params)?;
    // 只携带该候选自己的 binder；值可能是刚性泛型体里的开放 binder，实例化侧代入后才可用。
    let associated_binder_solutions = candidate
        .generic_binders
        .iter()
        .filter_map(|binder| substitutions.get(binder).cloned().map(|solution| (binder.clone(), solution)))
        .collect::<BTreeMap<_, _>>();
    Some(OverloadCandidate {
        declaration: candidate.declaration,
        instance: candidate.instance,
        sum_owner: candidate.sum_owner,
        sum_variant: candidate.sum_variant,
        symbol: candidate.symbol.clone(),
        owner: candidate.owner.clone(),
        impl_target: candidate.impl_target.clone(),
        trait_owner: candidate.trait_owner.clone(),
        domain: candidate.domain.clone(),
        signature: crate::compiler::hir::overload::diagnostics::OverloadSignature {
            params: expected_params,
            return_type: type_relations.resolve_associated_projections(
                &candidate.signature.return_type,
                &candidate.where_constraints,
                &substitutions,
            ),
        },
        match_kind,
        param_specs: candidate.param_specs.clone(),
        generic_binders: candidate.generic_binders.clone(),
        generic_binder_kinds: candidate.generic_binder_kinds.clone(),
        where_constraints: candidate.where_constraints.clone(),
        imported_dependency_export: candidate.imported_dependency_export,
        is_auto_constructor: candidate.is_auto_constructor,
        associated_binder_solutions,
    })
}

/// 把方法派发的结构化歧义登记到当前调用点。
///
/// `NoMatch` 不在此处登记：由上层通用兜底给出「无一候选」。`Ambiguous` 是独立失败
/// 类别，必须按调用点 span 冻结，否则会被兜底覆盖成「none matches」。
fn record_method_resolution_failure(span: &SourceSpan, callee: String, error: OverloadResolutionError) {
    let OverloadResolutionError::Ambiguous { candidates } = error
    else {
        return;
    };
    record_failure(HirCallResolutionFailure {
        span: span.clone(),
        callee,
        reason: HirCallResolutionFailureKind::AmbiguousExplicitImpls { candidates: candidates.len() },
    });
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
    call_span: &SourceSpan,
) -> Option<HirResolvedCall> {
    let (explicit_generic_arguments, callee) = peel_explicit_generic_arguments(callee);
    if let Some(local_name) = match &callee.kind {
        HirExprKind::Variable(identifier) => Some(identifier.name.as_str()),
        HirExprKind::Path(path) if path.parts().len() == 1 => Some(path.parts()[0].as_str()),
        _ => None,
    } {
        if let Some(ValkyrieType::Function(func)) = locals.get(local_name) {
            return Some(function_value_call(NamePath::new(vec![Identifier::new(local_name)]), func));
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
                    // 只收 trait 声明里的方法槽（无 owner）：impl 成员的 `trait_owner`
                    // 归属事实让它在名字与 trait 身份上都命中，但它不是槽位——
                    // 代入槽位声明会错配实参，必须把 impl 成员排除在这条路径之外。
                    for candidate in candidates
                        .iter()
                        .filter(|candidate| !candidate_has_receiver_parameter(candidate))
                        .filter(|candidate| candidate.owner.is_none())
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
                let mut call = HirResolvedCall {
                    declaration: matched.declaration,
                    instance: matched.instance,
                    sum_owner: matched.sum_owner,
                    sum_variant: matched.sum_variant,
                    trait_dispatch: None,
                    symbol: matched.symbol,
                    domain: HirCallableDomain::Function,
                    return_type: matched.signature.return_type,
                    parameter_types: matched.signature.params,
                    has_receiver: false,
                    extractor_payload_type: None,
                    associated_binder_solutions: matched.associated_binder_solutions,
                };
                // 泛型静态 trait 调用没有实例身份：把「哪个 trait 的哪个方法槽必须在
                // subject 上成立」冻结成 evidence，不在此处单态化，也不伪造实例。
                if call.instance.is_none()
                    && let Some(trait_declaration) = matched.trait_owner
                    && let Some(method_declaration) = call.declaration
                {
                    call.trait_dispatch = Some(HirTraitDispatch { trait_declaration, method_declaration, self_type: subject.target.clone() });
                }
                return Some(call);
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
        return match try_resolve_instance_method(object, field, args, candidates, type_relations, locals, struct_fields, singleton_names) {
            Ok(call) => Some(call),
            // 方法层确实没有候选时，字段类型为函数值的成员按函数值调用结算；
            // 字段不是函数值仍返回 `None`，由上层给出结构化「无候选」。
            Err(OverloadResolutionError::NoMatch) => function_typed_field_call(object, field, locals, struct_fields, singleton_names),
            Err(error) => {
                record_method_resolution_failure(call_span, callee.kind.callable_label(), error);
                None
            }
        };
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
                    kind: HirExprKind::FieldAccess { object: Box::new(receiver), field: field.clone(), field_id: None, tuple_index: None },
                    span: span.clone(),
                };
            }
            return match try_resolve_instance_method(
                &receiver,
                method_name,
                args,
                candidates,
                type_relations,
                locals,
                struct_fields,
                singleton_names,
            ) {
                Ok(call) => Some(call),
                Err(error) => {
                    record_method_resolution_failure(call_span, callee.kind.callable_label(), error);
                    None
                }
            };
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
                return try_resolve_qualified_free_function(
                    path,
                    args,
                    &explicit_generic_arguments,
                    candidates,
                    type_relations,
                    locals,
                    struct_fields,
                    singleton_names,
                );
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
        match try_resolve_instance_method_with_context(
            &first.value,
            &callee_name,
            method_args,
            candidates,
            type_relations,
            locals,
            struct_fields,
            singleton_names,
            contextual_type,
        ) {
            Ok(resolved) => return Some(resolved),
            // 运算符成员同为方法派发：同具体度并列必须按调用点冻结成结构化歧义，
            // 不继续回落到自由函数，也不在这里补造实现。
            Err(error @ OverloadResolutionError::Ambiguous { .. }) => {
                record_method_resolution_failure(call_span, callee.kind.callable_label(), error);
                return None;
            }
            Err(OverloadResolutionError::NoMatch) => {}
        }
        // operator 成员必须由显式声明（`imply` / `trait`）及其已选 provider 驱动。
        // 此处解析不出候选时保持 `resolved: None`，由 HIR 调用合同验证阶段以
        // `HirError::UnresolvedCall`（selector / 上下文 / source / span）结构化失败，
        // 编译器不补造目标实现，也不按类型类别 + 运算符名推导 opcode。
    }

    let enclosing_module = type_relations.enclosing_module();
    // 裸无载荷变体实参（`f(None)`）没有自足类型，其身份只能由实参槽上下文收窄。
    // 同名候选在该槽给出不同形参类型时上下文已置空：此时按任一候选的形参收窄
    // 都是猜测，必须失败闭合，交由调用合同报 `UnresolvedCall`。
    let argument_contexts =
        crate::compiler::hir::overload::substitution::free_call_argument_contexts(callee, candidates, type_relations, locals, singleton_names);
    let conflicted_unit_variant_argument = argument_contexts.iter().enumerate().any(|(index, context)| {
        if context.is_some() {
            return false;
        }
        let Some(argument) = args.get(index)
        else {
            return false;
        };
        let name = match &argument.value.kind {
            HirExprKind::Variable(identifier) => &identifier.name,
            HirExprKind::Path(path) if path.parts().len() == 1 => &path.parts()[0],
            _ => return false,
        };
        candidates.iter().any(|candidate| {
            candidate.signature.params.get(index).is_some_and(|expected| type_relations.is_unit_variant_of(expected, name))
        })
    });
    if conflicted_unit_variant_argument {
        return None;
    }
    let filtered = candidates
        .iter()
        .filter(|candidate| candidate.domain == OverloadDomain::Function && candidate.owner.is_none())
        .filter(|candidate| {
            symbol_matches_unqualified_free_call(&candidate.symbol, &callee_name, enclosing_module)
                || symbol_matches_imported_dependency_export(candidate, &callee_name)
                || type_relations.symbol_matches_import(&candidate.symbol, &callee_name)
                || type_relations.symbol_matches_group_scope(&candidate.symbol, &callee_name)
        })
        .filter_map(|candidate| {
            match_call_candidate(candidate, args, &explicit_generic_arguments, type_relations, locals, struct_fields, singleton_names)
        })
        .collect::<Vec<_>>();
    // 自由函数候选同样不得把同具体度并列折算成「无一候选」：组作用域会把
    // 同包不同 namespace 的同名声明一并拉进候选池，必须按调用点冻结成
    // 结构化歧义，不能按声明顺序静默挑一个。
    let resolved = match crate::compiler::hir::overload::diagnostics::resolve_overload(&filtered) {
        Ok(resolved) => resolved,
        Err(OverloadResolutionError::Ambiguous { candidates }) => {
            record_failure(HirCallResolutionFailure {
                span: call_span.clone(),
                callee: callee.kind.callable_label(),
                reason: HirCallResolutionFailureKind::AmbiguousFreeFunctionCandidates {
                    name: callee_name.to_string(),
                    candidates: candidates.len(),
                },
            });
            return None;
        }
        Err(OverloadResolutionError::NoMatch) => return None,
    };
    let has_receiver = candidate_has_receiver_parameter(&resolved);
    let symbol = overload_symbol_path(&resolved);
    let return_type = resolved.signature.return_type;
    let mut call = HirResolvedCall {
        declaration: resolved.declaration,
        instance: resolved.instance,
        sum_owner: resolved.sum_owner,
        sum_variant: resolved.sum_variant,
        trait_dispatch: None,
        symbol,
        domain: HirCallableDomain::Function,
        return_type,
        parameter_types: resolved.signature.params,
        has_receiver,
        extractor_payload_type: None,
        associated_binder_solutions: resolved.associated_binder_solutions,
    };
    Some(call)
}
