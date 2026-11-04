//! 表达式/语句/模式级调用解析（不含 try_resolve_* 核心匹配逻辑）。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::{
        control_flow_validation::display_type,
        overload::{
            call_resolver::{
                explain_unresolved_call, try_resolve_instance_method, try_resolve_nullary_constructor, try_resolve_pattern_extractor,
                resolve_variant_pattern_constructor, try_resolve_try_propagate_ok_variant,
            },
            diagnostics::{OverloadCandidate, OverloadDomain, OverloadResolutionError},
            expr_position::ExprPosition,
            symbol_match::{aggregate_constructor_matches_name, symbol_matches_unqualified_free_call},
            types::{
                canonical_extractor_callee, field_identity_from_type, infer_catch_scrutinee_type, infer_expr_type, infer_scrutinee_type,
                lookup_struct_field_type, normalize_local_field_chain, object_field_scrutinee_type, payload_field_scrutinee_types,
                should_resolve_name_pattern_as_type,
            },
        },
        type_relation::TypeRelationContext,
    },
    mir::{AggregateFieldIndex, collect_aggregate_field_map},
    types::{
        Identifier, NamePath,
        hir::{
            FunctionType, HirBlock, HirCallArgument, HirCallResolutionFailure, HirCallResolutionFailureKind, HirExpr, HirExprKind, HirFunction, HirFunctionReference,
            HirMatchArm, HirModule, HirLetOrigin, HirPattern, HirResolvedCall, HirStatement, HirStatementKind, ValkyrieType,
        },
    },
};

use super::{
    failure_sink::{failure_recorded_at, record_failure, replace_failure_at, with_failure_collector},
    validate::{is_option_owner_type, is_result_owner_type},
};

/// 对模块内全部 HIR 调用执行重载解析与绑定。
pub fn resolve_hir_calls(module: &mut HirModule) {
    let candidates = crate::compiler::hir::overload::candidates::collect_module_candidates(module);
    let type_relations = TypeRelationContext::from_module(module);
    let struct_fields = collect_aggregate_field_map(module);
    let singleton_names = module.singletons.iter().map(|singleton| singleton.name.clone()).collect::<std::collections::BTreeSet<_>>();

    resolve_submodule_calls(module, &candidates, &type_relations, &struct_fields, &singleton_names);
}

fn resolve_submodule_calls(
    module: &mut HirModule,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) {
    let type_relations = &type_relations.with_enclosing_module(&module.name);
    // 本层函数体内解析失败的调用点归本模块的注册表；子模块递归各归各层。
    let ((), failures) = with_failure_collector(|| {
        for function in &mut module.functions {
            resolve_function_calls(function, candidates, type_relations, struct_fields, singleton_names, None);
        }
        for item in &mut module.structs {
            use crate::compiler::hir::overload::types::struct_apply_type;
            let owner_type = struct_apply_type(item);
            for method in &mut item.methods {
                resolve_function_calls(method, candidates, type_relations, struct_fields, singleton_names, Some(&owner_type));
            }
        }
        for item in &mut module.singletons {
            for method in &mut item.methods {
                let owner_type = ValkyrieType::Named(item.name.clone());
                resolve_function_calls(method, candidates, type_relations, struct_fields, singleton_names, Some(&owner_type));
            }
            if let Some(constructor) = &mut item.constructor {
                let owner_type = ValkyrieType::Named(item.name.clone());
                resolve_function_calls(constructor, candidates, type_relations, struct_fields, singleton_names, Some(&owner_type));
            }
            if let Some(finalizer) = &mut item.finalizer {
                let owner_type = ValkyrieType::Named(item.name.clone());
                resolve_function_calls(finalizer, candidates, type_relations, struct_fields, singleton_names, Some(&owner_type));
            }
        }
        for item in &mut module.traits {
            let scoped =
                (!item.declaring_namespace.parts().is_empty()).then(|| type_relations.with_enclosing_module(&item.declaring_namespace));
            let relations = scoped.as_ref().unwrap_or(type_relations);
            for method in item.methods.iter_mut().chain(item.default_methods.iter_mut()) {
                resolve_function_calls(method, candidates, relations, struct_fields, singleton_names, None);
            }
        }
        for item in &mut module.impls {
            let scoped =
                (!item.declaring_namespace.parts().is_empty()).then(|| type_relations.with_enclosing_module(&item.declaring_namespace));
            let relations = scoped.as_ref().unwrap_or(type_relations);
            for method in &mut item.methods {
                resolve_function_calls(method, candidates, relations, struct_fields, singleton_names, Some(&item.target));
            }
        }
        for submodule in &mut module.submodules {
            resolve_submodule_calls(submodule, candidates, type_relations, struct_fields, singleton_names);
        }
    });
    module.call_resolution_failures.extend(failures);
}

fn resolve_function_calls(
    function: &mut HirFunction,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    owner_type: Option<&ValkyrieType>,
) {
    let type_relations = match function.declaring_namespace.parts() {
        [] => type_relations.clone(),
        _ => type_relations.with_enclosing_module(&function.declaring_namespace),
    };
    let type_relations = type_relations.with_callable_constraints(&function.where_constraints);
    let owned_relations;
    let type_relations = match owner_type {
        Some(owner) => {
            owned_relations = type_relations.with_impl_owner(owner.clone());
            &owned_relations
        }
        None => &type_relations,
    };
    let mut locals = function
        .params
        .iter()
        .map(|param| {
            let ty = if matches!(param.ty, ValkyrieType::r#SelfType)
                || (param.name.name.as_str() == "self" && matches!(param.ty, ValkyrieType::AutoType))
            {
                owner_type.cloned().unwrap_or(param.ty.clone())
            }
            else {
                param.ty.clone()
            };
            (param.name.name.to_string(), ty)
        })
        .collect::<BTreeMap<_, _>>();
    let contextual_type = Some(&function.return_type);
    resolve_block_calls(&mut function.body, candidates, &type_relations, &mut locals, struct_fields, singleton_names, contextual_type);
}

fn resolve_block_calls(
    block: &mut HirBlock,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &mut BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) {
    for statement in &mut block.statements {
        resolve_statement_calls(statement, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
    }
    if let Some(expr) = &mut block.expr {
        resolve_expr_calls(expr, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
    }
}

fn resolve_statement_calls(
    statement: &mut HirStatement,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &mut BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) {
    match &mut statement.kind {
        HirStatementKind::Let { pattern, initializer, ty, origin, .. } => {
            if let Some(initializer) = initializer {
                let hint = ty.as_ref().or(contextual_type);
                resolve_expr_calls(initializer, candidates, type_relations, locals, struct_fields, singleton_names, hint);
                // `loop item in src` 的迭代器绑定带结构化标记，不按临时名识别。
                // 语义义务只由 trait 声明身份结算：脱糖调用必须由 `IntoIterator`
                // 提供证据；同名的 `Iterator` 证据、同名 inherent 方法都不构成迭代源。
                if *origin == HirLetOrigin::ForInIterator
                    && let Some(source) = for_in_iterator_source(initializer).cloned()
                {
                    if !into_iterator_has_protocol_evidence(initializer, candidates, type_relations) {
                        // 已解析到同名 inherent 方法时也必须撤销：它不是协议实现。
                        if let HirExprKind::Call { resolved, .. } = &mut initializer.kind {
                            *resolved = None;
                        }
                        let source_type = infer_scrutinee_type(&source, locals, struct_fields, singleton_names);
                        let receiver = source_type.as_ref().map(display_type).unwrap_or_else(|| "<unknown>".to_string());
                        let failure = HirCallResolutionFailure {
                            span: initializer.span.clone(),
                            callee: "for-in".to_string(),
                            reason: HirCallResolutionFailureKind::ForInProtocolUnsatisfied { source: receiver },
                        };
                        replace_failure_at(&initializer.span, failure);
                    }
                }
            }
            // 方法体内 `let x: Self = ...`：`Self` 是 impl 目标，不能带着投影存活到
            // 成员调用匹配（否则 `Self` 与 `ArrayList<T>` 不统一，`push` 落空）。
            let binding_type = ty
                .as_ref()
                .map(|ty| match type_relations.impl_owner() {
                    Some(owner) => crate::compiler::hir::overload::substitute_type_self(ty, owner),
                    None => ty.clone(),
                })
                .or_else(|| initializer.as_ref().and_then(|expr| infer_scrutinee_type(expr, locals, struct_fields, singleton_names)));
            if let Some(binding_type) = binding_type {
                bind_pattern_type(pattern, &binding_type, locals);
            }
        }
        HirStatementKind::Expr(expr) => {
            resolve_expr_calls(expr, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type)
        }
    }
}

/// for-in 脱糖绑定的迭代源：`<source>.into_iterator()` 的接收者。
fn for_in_iterator_source(expr: &HirExpr) -> Option<&HirExpr> {
    let HirExprKind::Call { callee, args, .. } = &expr.kind
    else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let HirExprKind::FieldAccess { object, field, .. } = &callee.kind
    else {
        return None;
    };
    (field.as_str() == crate::compiler::semantic::operator_members::FOR_IN_INTO_ITERATOR_MEMBER).then_some(object.as_ref())
}

/// 脱糖调用是否真的由 `std.iterator::IntoIterator` 提供证据。
///
/// 具体 impl 成员按声明身份在候选表里核对 trait 归属；泛型源经 trait 槽解析时核对
/// witness 冻结的 trait 声明。同名 inherent 方法不构成证据。
fn into_iterator_has_protocol_evidence(
    initializer: &HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
) -> bool {
    let HirExprKind::Call { resolved: Some(call), .. } = &initializer.kind
    else {
        return false;
    };
    let Some(protocol) =
        type_relations.protocol_trait_declaration(crate::compiler::semantic::operator_members::FOR_IN_INTO_ITERATOR_TRAIT)
    else {
        return false;
    };
    if let Some(dispatch) = &call.trait_dispatch {
        return dispatch.trait_declaration == protocol;
    }
    candidates
        .iter()
        .find(|candidate| candidate.declaration == call.declaration)
        .and_then(|candidate| candidate.trait_owner)
        .is_some_and(|owner| owner == protocol)
}

/// 值位置成员访问（`receiver.member`，不带括号）的调用合同。
fn try_resolve_value_member_access(
    expr: &mut HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &mut BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) -> bool {
    let HirExprKind::FieldAccess { object, field, .. } = &mut expr.kind
    else {
        return false;
    };
    resolve_expr_calls(object, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
    let receiver = object.as_ref().clone();
    let member = field.clone();
    let span = expr.span.clone();
    let receiver_type = infer_expr_type(&receiver, locals).or_else(|| infer_scrutinee_type(&receiver, locals, struct_fields, singleton_names));
    if receiver_type.as_ref().is_some_and(|ty| lookup_struct_field_type(ty, &member, struct_fields).is_some()) {
        return false;
    }
    let resolved =
        match try_resolve_instance_method(&receiver, &member, &[], candidates, type_relations, locals, struct_fields, singleton_names) {
            Ok(call) => Some(call),
            // 同具体度并列是独立失败类别：不得回落成自由函数调用，也不得留在字段访问形态。
            // 冻结成 `resolved: None` 的调用形态，让合同校验按该 span 取到结构化原因。
            Err(OverloadResolutionError::Ambiguous { candidates: ambiguous }) => {
                let callee = HirExpr { kind: HirExprKind::Path(NamePath::new(vec![member.clone()])), span: span.clone() };
                let args = vec![HirCallArgument::positional(receiver)];
                record_failure(HirCallResolutionFailure {
                    span: span.clone(),
                    callee: member.to_string(),
                    reason: HirCallResolutionFailureKind::AmbiguousExplicitImpls { candidates: ambiguous.len() },
                });
                expr.kind = HirExprKind::Call { callee: Box::new(callee), args, resolved: None };
                return true;
            }
            Err(OverloadResolutionError::NoMatch) => {
                let callee = HirExpr { kind: HirExprKind::Path(NamePath::new(vec![member.clone()])), span: span.clone() };
                let args = vec![HirCallArgument::positional(receiver.clone())];
                call_resolver_try_resolve_call(
                    &callee,
                    &args,
                    candidates,
                    type_relations,
                    locals,
                    struct_fields,
                    singleton_names,
                    contextual_type,
                    &span,
                )
            }
        };
    let Some(resolved) = resolved
    else {
        return false;
    };
    let callee = HirExpr { kind: HirExprKind::Path(NamePath::new(vec![member])), span };
    let args = vec![HirCallArgument::positional(receiver)];
    expr.kind = HirExprKind::Call { callee: Box::new(callee), args, resolved: Some(resolved) };
    true
}

fn resolve_expr_calls(
    expr: &mut HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &mut BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) {
    resolve_expr_calls_at(expr, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type, ExprPosition::Value);
}

/// 裸名实参唯一命中非泛型自由函数声明时改写为 `FunctionReference`。
///
/// 判定全部来自候选的结构事实：无限定符号路径与当前模块精确对齐、`declaration`
/// 已冻结、`generic_binders` 为空。局部绑定与单例名优先，不参与改写；泛型、歧义
/// 或未命中一律保持原表达式，让调用沿既有路径失败，禁止按短名猜一个实现。
fn rewrite_free_function_argument_to_reference(
    expr: &mut HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) {
    let name = match &expr.kind {
        HirExprKind::Variable(identifier) => identifier.name.clone(),
        HirExprKind::Path(path) if path.parts().len() == 1 => path.parts()[0].clone(),
        _ => return,
    };
    if locals.contains_key(name.as_str()) || singleton_names.contains(&name) {
        return;
    }
    let enclosing_module = type_relations.enclosing_module();
    let mut matched: Option<&OverloadCandidate> = None;
    for candidate in candidates {
        if candidate.domain != OverloadDomain::Function || candidate.owner.is_some() {
            continue;
        }
        if candidate.declaration.is_none() || !candidate.generic_binders.is_empty() {
            continue;
        }
        if !symbol_matches_unqualified_free_call(&candidate.symbol, &name, enclosing_module) {
            continue;
        }
        if matched.is_some() {
            return;
        }
        matched = Some(candidate);
    }
    let Some(candidate) = matched
    else {
        return;
    };
    expr.kind = HirExprKind::FunctionReference(HirFunctionReference {
        declaration: candidate.declaration,
        instance: candidate.instance,
        symbol: candidate.symbol.clone(),
        signature: Box::new(FunctionType { params: candidate.signature.params.clone(), return_type: candidate.signature.return_type.clone() }),
    });
}

fn resolve_expr_calls_at(
    expr: &mut HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &mut BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
    position: ExprPosition,
) {
    try_resolve_nullary_variant_expr(expr, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
    normalize_local_field_chain(expr, locals, struct_fields);
    if position == ExprPosition::Value
        && try_resolve_value_member_access(expr, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type)
    {
        return;
    }
    // `expr.kind` 可变借用期间取不到 `expr.span`，失败登记需要的区间先捕获。
    let expr_span = expr.span.clone();
    match &mut expr.kind {
        HirExprKind::Call { callee, args, resolved } => {
            resolve_expr_calls_at(
                callee,
                candidates,
                type_relations,
                locals,
                struct_fields,
                singleton_names,
                contextual_type,
                ExprPosition::Callee,
            );
            let member_argument_contexts = crate::compiler::hir::overload::substitution::member_call_argument_contexts(
                callee,
                candidates,
                type_relations,
                locals,
                struct_fields,
                singleton_names,
            );
            // 成员调用形态供给为空时，按自由函数 callee 补同样的逐槽形参上下文；
            // 两种 callee 形态互斥，不叠加。
            let argument_contexts = if member_argument_contexts.is_empty() {
                crate::compiler::hir::overload::substitution::free_call_argument_contexts(callee, candidates, type_relations, locals, singleton_names)
            } else {
                member_argument_contexts
            };
            for (index, arg) in args.iter_mut().enumerate() {
                let supplied = argument_contexts.get(index).and_then(|context| context.as_ref());
                let context = if call_resolver_is_unresolved_variant_reference(&arg.value, locals) {
                    supplied.or(contextual_type)
                }
                else {
                    contextual_type
                };
                resolve_expr_calls(&mut arg.value, candidates, type_relations, locals, struct_fields, singleton_names, context);
                rewrite_free_function_argument_to_reference(&mut arg.value, candidates, type_relations, locals, singleton_names);
            }
            *resolved = call_resolver_try_resolve_call(
                callee,
                args,
                candidates,
                type_relations,
                locals,
                struct_fields,
                singleton_names,
                contextual_type,
                &expr_span,
            );
            // 解析期已按本调用点冻结了结构化失败（例如同具体度歧义）时保留该原因，
            // 不用宽泛的「无一候选」覆盖。
            if resolved.is_none() && !failure_recorded_at(&expr_span) {
                record_failure(HirCallResolutionFailure {
                    span: expr_span,
                    callee: callee.kind.callable_label(),
                    reason: explain_unresolved_call(callee, args, candidates, type_relations, locals, struct_fields, singleton_names),
                });
            }
        }
        HirExprKind::FieldAccess { object, field, field_id, .. } => {
            resolve_expr_calls(object, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            if field_id.is_none() {
                *field_id = infer_scrutinee_type(object, locals, struct_fields, singleton_names)
                    .as_ref()
                    .and_then(|object_type| field_identity_from_type(object_type, field, struct_fields));
            }
        }
        HirExprKind::GenericApply { callee, .. } => resolve_expr_calls_at(
            callee,
            candidates,
            type_relations,
            locals,
            struct_fields,
            singleton_names,
            contextual_type,
            ExprPosition::Callee,
        ),
        HirExprKind::FieldInit { value: callee, .. }
        | HirExprKind::Await(callee)
        | HirExprKind::Awake(callee)
        | HirExprKind::BlockOn(callee)
        | HirExprKind::YieldFrom(callee)
        | HirExprKind::Raise(callee)
        | HirExprKind::Resume(callee)
        | HirExprKind::Assign { value: callee, .. } => {
            resolve_expr_calls(callee, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type)
        }
        HirExprKind::StoreField { object, field, value, field_id } => {
            resolve_expr_calls(object, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            resolve_expr_calls(value, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            if field_id.is_none() {
                *field_id = infer_scrutinee_type(object, locals, struct_fields, singleton_names)
                    .as_ref()
                    .and_then(|object_type| field_identity_from_type(object_type, field, struct_fields));
            }
        }
        HirExprKind::ArrayNew { length, .. } => {
            resolve_expr_calls(length, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type)
        }
        HirExprKind::ArrayLiteral { items: args }
        | HirExprKind::TupleLiteral { items: args }
        | HirExprKind::CollectionInit { items: args, .. } => {
            for arg in args {
                resolve_expr_calls(arg, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            }
        }
        HirExprKind::TryPropagate { inner, resolved_ok_variant } => {
            resolve_expr_calls(inner, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            if resolved_ok_variant.is_none() {
                let operand_type = infer_scrutinee_type(inner, locals, struct_fields, singleton_names);
                *resolved_ok_variant = operand_type.as_ref().and_then(|ty| try_resolve_try_propagate_ok_variant(ty, candidates));
            }
        }
        HirExprKind::Construct { path, generic_arguments, args, resolved } => {
            let constructor_name = path.parts().last().cloned();
            for arg in args.iter_mut() {
                let field_context = match &arg.kind {
                    HirExprKind::FieldInit { name: field_name, .. } => constructor_name
                        .as_ref()
                        .and_then(|name| struct_fields.fields_of(&ValkyrieType::Named(name.clone())))
                        .and_then(|fields| fields.iter().find(|field| field.name == *field_name).map(|field| &field.ty))
                        .or_else(|| {
                            candidates.iter().filter(|candidate| aggregate_constructor_matches_name(candidate, path)).find_map(|candidate| {
                                candidate.param_specs.iter().find(|param| param.name.name == *field_name).map(|param| &param.ty)
                            })
                        }),
                    _ => None,
                };
                if let HirExprKind::FieldInit { value, .. } = &mut arg.kind {
                    resolve_expr_calls(
                        value,
                        candidates,
                        type_relations,
                        locals,
                        struct_fields,
                        singleton_names,
                        field_context.or(contextual_type),
                    );
                }
                else {
                    resolve_expr_calls(arg, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
                }
            }
            if resolved.is_none() {
                *resolved = call_resolver_try_resolve_constructor(
                    path,
                    generic_arguments,
                    args,
                    candidates,
                    type_relations,
                    locals,
                    struct_fields,
                    singleton_names,
                    contextual_type,
                );
            }
        }
        HirExprKind::Cast { expr, .. } => {
            resolve_expr_calls(expr, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
        }
        HirExprKind::Block(block) => {
            resolve_block_calls(block, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type)
        }
        HirExprKind::TryScope { body, .. } => {
            resolve_block_calls(body, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type)
        }
        HirExprKind::Lambda { params, body, .. } => {
            let mut lambda_locals = locals.clone();
            for param in params {
                lambda_locals.insert(param.name.name.to_string(), param.ty.clone());
            }
            resolve_block_calls(body, candidates, type_relations, &mut lambda_locals, struct_fields, singleton_names, None);
        }
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                resolve_expr_calls(value, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            }
            for method in methods {
                resolve_function_calls(method, candidates, type_relations, struct_fields, singleton_names, None);
            }
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            resolve_expr_calls(condition, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            let mut then_locals = locals.clone();
            resolve_block_calls(then_branch, candidates, type_relations, &mut then_locals, struct_fields, singleton_names, contextual_type);
            if let Some(else_branch) = else_branch {
                let mut else_locals = locals.clone();
                resolve_block_calls(else_branch, candidates, type_relations, &mut else_locals, struct_fields, singleton_names, contextual_type);
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            resolve_expr_calls(scrutinee, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            let scrutinee_type = infer_scrutinee_type(scrutinee, locals, struct_fields, singleton_names);
            for arm in arms {
                resolve_arm_calls(
                    arm,
                    candidates,
                    type_relations,
                    locals,
                    scrutinee_type.as_ref(),
                    struct_fields,
                    singleton_names,
                    contextual_type,
                );
            }
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            resolve_expr_calls(scrutinee, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            let scrutinee_type = infer_scrutinee_type(scrutinee, locals, struct_fields, singleton_names);
            resolve_pattern_calls(pattern, candidates, type_relations, scrutinee_type.as_ref(), struct_fields);
            let mut then_locals = locals.clone();
            if let Some(scrutinee_type) = scrutinee_type.as_ref() {
                bind_pattern_type(pattern, scrutinee_type, &mut then_locals);
            }
            resolve_block_calls(then_branch, candidates, type_relations, &mut then_locals, struct_fields, singleton_names, contextual_type);
            if let Some(else_branch) = else_branch {
                let mut else_locals = locals.clone();
                resolve_block_calls(else_branch, candidates, type_relations, &mut else_locals, struct_fields, singleton_names, contextual_type);
            }
        }
        HirExprKind::Loop { pattern, iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                resolve_expr_calls(iterator, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            }
            if let Some(condition) = condition {
                resolve_expr_calls(condition, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            }
            let mut body_locals = locals.clone();
            if let (Some(pattern), Some(iterator)) = (pattern, iterator) {
                let iter_type = infer_expr_type(iterator, locals);
                resolve_pattern_calls(pattern, candidates, type_relations, iter_type.as_ref(), struct_fields);
                // for-in 在 HIR 下降期已脱糖为迭代协议调用（`into_iterator`/`next`/`unwrap`），
                // 迭代项类型由协议成员返回类型普通推导；只有结构化数组 `[T]` 仍在此直接给元素类型，
                // 集合聚合一律不得把自身绑成元素类型。
                if let Some(ValkyrieType::Array(inner)) = iter_type.as_ref() {
                    let element_type = inner.as_ref().clone();
                    bind_pattern_type(pattern, &element_type, &mut body_locals);
                }
            }
            resolve_block_calls(body, candidates, type_relations, &mut body_locals, struct_fields, singleton_names, contextual_type);
        }
        HirExprKind::Return(Some(value)) | HirExprKind::Break { expr: Some(value), .. } | HirExprKind::Yield(Some(value)) => {
            resolve_expr_calls(value, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
        }
        HirExprKind::Catch { expr, arms } => {
            resolve_expr_calls(expr, candidates, type_relations, locals, struct_fields, singleton_names, contextual_type);
            let scrutinee_type = infer_catch_scrutinee_type(expr, locals);
            for arm in arms {
                resolve_arm_calls(
                    arm,
                    candidates,
                    type_relations,
                    locals,
                    scrutinee_type.as_ref(),
                    struct_fields,
                    singleton_names,
                    contextual_type,
                );
            }
        }
        HirExprKind::Variable(_)
        | HirExprKind::Literal(_)
        | HirExprKind::Path(_)
        | HirExprKind::FunctionReference(_)
        | HirExprKind::Return(None)
        | HirExprKind::Break { expr: None, .. }
        | HirExprKind::Continue { .. }
        | HirExprKind::Yield(None)
        | HirExprKind::Fallthrough
        | HirExprKind::SuperCall { .. }
        | HirExprKind::With { .. } => {}
    }
}

fn resolve_arm_calls(
    arm: &mut HirMatchArm,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    scrutinee_type: Option<&ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) {
    let mut arm_locals = locals.clone();
    resolve_pattern_calls(&mut arm.pattern, candidates, type_relations, scrutinee_type, struct_fields);
    if let Some(scrutinee_type) = scrutinee_type {
        bind_pattern_type(&arm.pattern, scrutinee_type, &mut arm_locals);
    }
    if let Some(guard) = &mut arm.guard {
        resolve_expr_calls(guard, candidates, type_relations, &mut arm_locals, struct_fields, singleton_names, contextual_type);
    }
    resolve_expr_calls(&mut arm.body, candidates, type_relations, &mut arm_locals, struct_fields, singleton_names, contextual_type);
}

fn resolve_pattern_calls(
    pattern: &mut HirPattern,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    scrutinee_type: Option<&ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
) {
    match pattern {
        HirPattern::Name(name) => {
            if name.parts().len() == 1 {
                let first_part = name.parts().first().expect("single-segment name should have first part");
                if first_part.as_str().chars().next().is_some_and(|c| c.is_lowercase()) {
                    unreachable!(
                        "单段小写名字模式 `{}` 应由解析器处理为 Variable，不应出现在 Name 形式中",
                        name.parts().iter().map(|p| p.as_str()).collect::<Vec<_>>().join("::")
                    );
                }
            }
            if let Some(actual_type) = scrutinee_type {
                if should_resolve_name_pattern_as_type(name, actual_type) {
                    *pattern = HirPattern::Type {
                        path: name.clone(),
                        declaration: crate::compiler::hir::overload::types::pattern_type_declaration(name, type_relations),
                    };
                }
                else {
                    let canonical_callee = canonical_extractor_callee(name);
                    let resolved = try_resolve_pattern_extractor(&canonical_callee, actual_type, candidates, type_relations)
                        .or_else(|| resolve_variant_pattern_constructor(&canonical_callee, actual_type, candidates));
                    if let Some(resolved) = resolved {
                        *pattern = HirPattern::Extractor(crate::compiler::types::hir::HirExtractorPattern::Constructor {
                            name: name.clone(),
                            canonical_callee,
                            fields: Vec::new(),
                            resolved: Some(resolved),
                        });
                    }
                }
            }
        }
        HirPattern::Extractor(crate::compiler::types::hir::HirExtractorPattern::Constructor { fields, canonical_callee, resolved, .. }) => {
            if let Some(actual_type) = scrutinee_type {
                *resolved = try_resolve_pattern_extractor(canonical_callee, actual_type, candidates, type_relations)
                    .or_else(|| resolve_variant_pattern_constructor(canonical_callee, actual_type, candidates));
            }
            let field_scrutinee_types = resolved
                .as_ref()
                .and_then(|resolved| resolved.extractor_payload_type.as_ref())
                .map(|payload| payload_field_scrutinee_types(payload, struct_fields))
                .unwrap_or_default();
            for (index, field) in fields.iter_mut().enumerate() {
                let field_scrutinee = field_scrutinee_types.get(index).and_then(|ty| ty.as_ref());
                resolve_pattern_calls(field, candidates, type_relations, field_scrutinee, struct_fields);
            }
        }
        HirPattern::Extractor(crate::compiler::types::hir::HirExtractorPattern::Array {
            prefix, suffix, canonical_callee, resolved, ..
        }) => {
            for item in prefix.iter_mut() {
                resolve_pattern_calls(item, candidates, type_relations, None, struct_fields);
            }
            for item in suffix.iter_mut() {
                resolve_pattern_calls(item, candidates, type_relations, None, struct_fields);
            }
            if let Some(actual_type) = scrutinee_type {
                *resolved = try_resolve_pattern_extractor(canonical_callee, actual_type, candidates, type_relations);
            }
        }
        HirPattern::Tuple(items) => {
            for item in items.iter_mut() {
                resolve_pattern_calls(item, candidates, type_relations, None, struct_fields);
            }
        }
        HirPattern::Or(items) => {
            for item in items.iter_mut() {
                resolve_pattern_calls(item, candidates, type_relations, scrutinee_type, struct_fields);
            }
        }
        HirPattern::Object { name, type_declaration, fields, field_ids, rest, .. } => {
            if rest.is_none() {
                if let Some(name_path) = name.clone() {
                    if let Some(actual_type) = scrutinee_type {
                        let canonical_callee = canonical_extractor_callee(&name_path);
                        let resolved = try_resolve_pattern_extractor(&canonical_callee, actual_type, candidates, type_relations)
                            .or_else(|| resolve_variant_pattern_constructor(&canonical_callee, actual_type, candidates));
                        if resolved.is_some() {
                            let extractor_fields = fields.iter().map(|(_, field)| field.clone()).collect::<Vec<_>>();
                            *pattern = HirPattern::Extractor(crate::compiler::types::hir::HirExtractorPattern::Constructor {
                                name: name_path.clone(),
                                canonical_callee,
                                fields: extractor_fields,
                                resolved,
                            });
                            if let HirPattern::Extractor(crate::compiler::types::hir::HirExtractorPattern::Constructor {
                                fields,
                                resolved,
                                ..
                            }) = pattern
                            {
                                if let Some(payload) = resolved
                                    .as_ref()
                                    .and_then(|call| call.extractor_payload_type.clone())
                                    .or_else(|| crate::compiler::hir::try_propagate::nullable_payload_type(actual_type))
                                {
                                    let field_types = payload_field_scrutinee_types(&payload, struct_fields);
                                    for (index, field) in fields.iter_mut().enumerate() {
                                        let field_scrutinee = field_types.get(index).and_then(|ty| ty.as_ref());
                                        resolve_pattern_calls(field, candidates, type_relations, field_scrutinee, struct_fields);
                                    }
                                }
                                else if is_option_owner_type(actual_type) || is_result_owner_type(actual_type) {
                                    for field in fields.iter_mut() {
                                        resolve_pattern_calls(field, candidates, type_relations, None, struct_fields);
                                    }
                                }
                            }
                            return;
                        }
                    }
                }
            }
            if type_declaration.is_none() {
                *type_declaration = name
                    .as_ref()
                    .and_then(|name_path| crate::compiler::hir::overload::types::pattern_type_declaration(name_path, type_relations));
            }
            let field_owner = type_declaration
                // 字段模式解析的 owner 只可能是带字段的聚合；`[primitive]` 声明零字段，
                // 不会进入字段解析，这里没有自举身份可查。
                .map(|declaration| ValkyrieType::Nominal { declaration, arguments: Vec::new(), primitive_identity: None })
                .or_else(|| scrutinee_type.cloned());
            for (index, (field_name, item)) in fields.iter_mut().enumerate() {
                let field_scrutinee = object_field_scrutinee_type(name.as_ref(), field_name, struct_fields);
                resolve_pattern_calls(item, candidates, type_relations, field_scrutinee, struct_fields);
                if let Some(slot) = field_ids.get_mut(index) {
                    if slot.is_none() {
                        *slot = field_owner.as_ref().and_then(|owner| field_identity_from_type(owner, field_name, struct_fields));
                    }
                }
            }
        }
        HirPattern::Bind { pattern, .. } | HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => {
            resolve_pattern_calls(pattern, candidates, type_relations, scrutinee_type, struct_fields);
        }
        _ => {}
    }
}

pub(crate) fn bind_pattern_type(pattern: &HirPattern, ty: &ValkyrieType, locals: &mut BTreeMap<String, ValkyrieType>) {
    match pattern {
        HirPattern::Variable(identifier) => {
            locals.insert(identifier.name.to_string(), ty.clone());
        }
        HirPattern::TypedBind { identifier, ty: bound_ty } => {
            locals.insert(identifier.name.to_string(), ValkyrieType::Named(bound_ty.name().clone()));
        }
        HirPattern::Tuple(items) if matches!(ty, ValkyrieType::Tuple(_)) => {
            if let ValkyrieType::Tuple(types) = ty {
                for (pattern, item_ty) in items.iter().zip(types) {
                    bind_pattern_type(pattern, item_ty, locals);
                }
            }
        }
        HirPattern::Bind { identifier, pattern } => {
            bind_pattern_type(pattern, ty, locals);
            locals.insert(identifier.name.to_string(), ty.clone());
        }
        HirPattern::Extractor(crate::compiler::types::hir::HirExtractorPattern::Constructor { name, fields, resolved, .. }) => {
            let variant = name.parts().first().map(|part| part.as_str());
            let payload = resolved
                .as_ref()
                .and_then(|call| call.extractor_payload_type.clone())
                .or_else(|| crate::compiler::hir::try_propagate::nullable_payload_type(ty))
                .or_else(|| crate::compiler::hir::try_propagate::option_apply_payload_type(ty))
                .or_else(|| variant.and_then(|variant| crate::compiler::hir::try_propagate::result_variant_payload_type(ty, variant)));
            if let Some(payload) = payload {
                for (index, field) in fields.iter().enumerate() {
                    let field_ty = match &payload {
                        ValkyrieType::Tuple(items) => items.get(index).cloned(),
                        _ if index == 0 => Some(payload.clone()),
                        _ => None,
                    };
                    if let Some(field_ty) = field_ty {
                        bind_pattern_type(field, &field_ty, locals);
                    }
                }
            }
        }
        HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => bind_pattern_type(pattern, ty, locals),
        _ => {}
    }
}

fn call_resolver_try_resolve_call(
    callee: &HirExpr,
    args: &[HirCallArgument],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
    call_span: &crate::compiler::types::SourceSpan,
) -> Option<HirResolvedCall> {
    crate::compiler::hir::overload::call_resolver::try_resolve_call(
        callee,
        args,
        candidates,
        type_relations,
        locals,
        struct_fields,
        singleton_names,
        contextual_type,
        call_span,
    )
}

fn call_resolver_try_resolve_constructor(
    path: &NamePath,
    explicit_generic_arguments: &[ValkyrieType],
    args: &[crate::compiler::types::hir::HirExpr],
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) -> Option<HirResolvedCall> {
    crate::compiler::hir::overload::call_resolver::try_resolve_constructor(
        path,
        explicit_generic_arguments,
        args,
        candidates,
        type_relations,
        locals,
        struct_fields,
        singleton_names,
        contextual_type,
    )
}

fn call_resolver_is_unresolved_variant_reference(expr: &HirExpr, locals: &BTreeMap<String, ValkyrieType>) -> bool {
    crate::compiler::hir::overload::call_resolver::is_unresolved_variant_reference(expr, locals)
}

fn try_resolve_nullary_variant_expr(
    expr: &mut HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) {
    let name = match &expr.kind {
        HirExprKind::Variable(identifier) if !locals.contains_key(identifier.name.as_str()) => identifier.name.clone(),
        HirExprKind::Path(path) if path.parts().len() == 1 && !locals.contains_key(path.parts()[0].as_str()) => path.parts()[0].clone(),
        _ => return,
    };
    if let Some(resolved) =
        try_resolve_nullary_constructor(&name, contextual_type, candidates, type_relations, locals, struct_fields, singleton_names)
    {
        *expr = HirExpr {
            span: expr.span.clone(),
            kind: HirExprKind::Construct {
                path: NamePath::new(vec![name.clone()]),
                generic_arguments: Vec::new(),
                args: Vec::new(),
                resolved: Some(resolved),
            },
        };
    }
}
