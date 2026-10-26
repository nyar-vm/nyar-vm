//! 类型代换与泛型统一。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::type_relation::{ParameterMatchResult, TypeRelationContext},
    mir::AggregateFieldIndex,
    types::{
        Identifier,
        hir::{GenericBinderId, HirAssociatedTypeBinding, HirParam, HirTraitBound, HirWhereConstraint, ValkyrieType},
    },
};

use super::diagnostics::OverloadCandidate;

/// 类型中是否仍有未代入的泛型 binder（含 nominal 实参中的 binder）。
///
/// 这类接收者无法在声明期唯一选择 impl；调用只能冻结成员要求，由单态化按
/// 完整代入选择 witness。
pub(crate) fn has_unresolved_generic_arguments(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Generic(generic) => generic.binder.is_some(),
        ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(has_unresolved_generic_arguments),
        ValkyrieType::Apply(base, arguments) => {
            has_unresolved_generic_arguments(base) || arguments.iter().any(has_unresolved_generic_arguments)
        }
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => has_unresolved_generic_arguments(element),
        ValkyrieType::FixedArray { element, .. } => has_unresolved_generic_arguments(element),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
            items.iter().any(has_unresolved_generic_arguments)
        }
        ValkyrieType::Function(function) => {
            function.params.iter().any(has_unresolved_generic_arguments) || has_unresolved_generic_arguments(&function.return_type)
        }
        _ => false,
    }
}

pub(crate) fn owner_type_generic_binders(ty: &ValkyrieType) -> std::collections::BTreeSet<GenericBinderId> {
    fn collect(ty: &ValkyrieType, binders: &mut std::collections::BTreeSet<GenericBinderId>) {
        match ty {
            ValkyrieType::Generic(generic) => {
                if let Some(binder) = generic.binder {
                    binders.insert(binder);
                }
            }
            ValkyrieType::Nominal { arguments, .. } | ValkyrieType::Tuple(arguments) => {
                for argument in arguments {
                    collect(argument, binders);
                }
            }
            ValkyrieType::Apply(base, arguments) => {
                collect(base, binders);
                for argument in arguments {
                    collect(argument, binders);
                }
            }
            ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => collect(element, binders),
            _ => {}
        }
    }
    let mut binders = std::collections::BTreeSet::new();
    collect(ty, &mut binders);
    binders
}

/// `self` 上的 `AutoType`/`Self` 必须在候选阶段换成完整 owner（可为 `Nominal{ItemId}`）。
/// 禁止留给 `match_call_candidate` 再收成 `Named(短名)`，否则与已绑定字段类型对不上。
pub(crate) fn specialize_method_param_type(
    ty: &ValkyrieType,
    index: usize,
    param_specs: &[HirParam],
    owner_type: &ValkyrieType,
) -> ValkyrieType {
    let is_self_slot = index == 0 && param_specs.first().is_some_and(|param| param.name.name.as_str() == "self");
    if is_self_slot && matches!(ty, ValkyrieType::AutoType | ValkyrieType::r#SelfType) {
        return owner_type.clone();
    }
    substitute_type_self(ty, owner_type)
}

/// 把类型中的 `SelfType` 替换为给定主体；外部测试入口。
#[doc(hidden)]
pub fn substitute_type_self(ty: &ValkyrieType, replacement: &ValkyrieType) -> ValkyrieType {
    match ty {
        ValkyrieType::SelfType => replacement.clone(),
        ValkyrieType::Nominal { declaration, arguments } => ValkyrieType::Nominal {
            declaration: *declaration,
            arguments: arguments.iter().map(|argument| substitute_type_self(argument, replacement)).collect(),
        },
        ValkyrieType::Array(inner) => ValkyrieType::Array(Box::new(substitute_type_self(inner, replacement))),
        ValkyrieType::Apply(base, args) => ValkyrieType::Apply(
            Box::new(substitute_type_self(base, replacement)),
            args.iter().map(|arg| substitute_type_self(arg, replacement)).collect(),
        ),
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(items.iter().map(|item| substitute_type_self(item, replacement)).collect()),
        ValkyrieType::Union(items) => ValkyrieType::Union(items.iter().map(|item| substitute_type_self(item, replacement)).collect()),
        ValkyrieType::Nullable(inner) => ValkyrieType::Nullable(Box::new(substitute_type_self(inner, replacement))),
        ValkyrieType::Associated(associated) => ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
            base: substitute_type_self(&associated.base, replacement),
            name: associated.name.clone(),
            type_arguments: associated.type_arguments.iter().map(|ty| substitute_type_self(ty, replacement)).collect(),
        })),
        other => other.clone(),
    }
}

/// Replace `Self` with the imply/class owner so instance dispatch is nominal.
pub(crate) fn substitute_self_type(ty: &ValkyrieType, owner: Option<&Identifier>) -> ValkyrieType {
    match ty {
        ValkyrieType::r#SelfType => match owner {
            Some(owner) => ValkyrieType::Named(owner.clone()),
            None => ty.clone(),
        },
        ValkyrieType::Array(inner) => ValkyrieType::Array(Box::new(substitute_self_type(inner, owner))),
        ValkyrieType::Apply(base, args) => {
            ValkyrieType::Apply(Box::new(substitute_self_type(base, owner)), args.iter().map(|arg| substitute_self_type(arg, owner)).collect())
        }
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(items.iter().map(|item| substitute_self_type(item, owner)).collect()),
        ValkyrieType::Union(items) => ValkyrieType::Union(items.iter().map(|item| substitute_self_type(item, owner)).collect()),
        ValkyrieType::Function(func) => ValkyrieType::Function(Box::new(crate::compiler::types::hir::FunctionType {
            params: func.params.iter().map(|param| substitute_self_type(param, owner)).collect(),
            return_type: substitute_self_type(&func.return_type, owner),
        })),
        other => other.clone(),
    }
}

/// 按关联类型等式替换 `Associated` 形状；外部测试入口。
#[doc(hidden)]
pub fn substitute_associated_equations(
    ty: &ValkyrieType,
    constrained_type: &ValkyrieType,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) -> ValkyrieType {
    match ty {
        ValkyrieType::Nominal { declaration, arguments } => ValkyrieType::Nominal {
            declaration: *declaration,
            arguments: arguments.iter().map(|argument| substitute_associated_equations(argument, constrained_type, equations)).collect(),
        },
        ValkyrieType::Associated(associated) if &associated.base == constrained_type => {
            equations.get(&associated.name).cloned().unwrap_or_else(|| {
                ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
                    base: substitute_associated_equations(&associated.base, constrained_type, equations),
                    name: associated.name.clone(),
                    type_arguments: associated
                        .type_arguments
                        .iter()
                        .map(|argument| substitute_associated_equations(argument, constrained_type, equations))
                        .collect(),
                }))
            })
        }
        ValkyrieType::Associated(associated) => ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
            base: substitute_associated_equations(&associated.base, constrained_type, equations),
            name: associated.name.clone(),
            type_arguments: associated
                .type_arguments
                .iter()
                .map(|argument| substitute_associated_equations(argument, constrained_type, equations))
                .collect(),
        })),
        ValkyrieType::Apply(base, arguments) => ValkyrieType::Apply(
            Box::new(substitute_associated_equations(base, constrained_type, equations)),
            arguments.iter().map(|argument| substitute_associated_equations(argument, constrained_type, equations)).collect(),
        ),
        ValkyrieType::Array(element) => ValkyrieType::Array(Box::new(substitute_associated_equations(element, constrained_type, equations))),
        ValkyrieType::FixedArray { element, length } => ValkyrieType::FixedArray {
            element: Box::new(substitute_associated_equations(element, constrained_type, equations)),
            length: *length,
        },
        ValkyrieType::Tuple(items) => {
            ValkyrieType::Tuple(items.iter().map(|item| substitute_associated_equations(item, constrained_type, equations)).collect())
        }
        ValkyrieType::Union(items) => {
            ValkyrieType::Union(items.iter().map(|item| substitute_associated_equations(item, constrained_type, equations)).collect())
        }
        ValkyrieType::Intersection(items) => {
            ValkyrieType::Intersection(items.iter().map(|item| substitute_associated_equations(item, constrained_type, equations)).collect())
        }
        ValkyrieType::Nullable(inner) => ValkyrieType::Nullable(Box::new(substitute_associated_equations(inner, constrained_type, equations))),
        ValkyrieType::Function(function) => ValkyrieType::Function(Box::new(crate::compiler::types::hir::FunctionType {
            params: function.params.iter().map(|param| substitute_associated_equations(param, constrained_type, equations)).collect(),
            return_type: substitute_associated_equations(&function.return_type, constrained_type, equations),
        })),
        other => other.clone(),
    }
}

/// 用 bound 的 trait 声明、实参与关联等式实例化成员签名。
pub(crate) fn instantiate_trait_bound_candidate(
    candidate: &OverloadCandidate,
    subject: &ValkyrieType,
    bound: &HirTraitBound,
) -> Option<OverloadCandidate> {
    let trait_id = bound.declaration?;
    if candidate.trait_owner != Some(trait_id) {
        return None;
    }
    let binders = candidate.generic_binders.iter().filter(|binder| binder.owner == trait_id).copied().collect::<Vec<_>>();
    if binders.len() != bound.type_arguments.len() {
        return None;
    }
    let substitution = binders.into_iter().zip(bound.type_arguments.iter().cloned()).collect();
    let mut candidate = substitute_trait_self(candidate, subject);
    let equations = bound.associated_types.iter().map(|equation| (equation.name.clone(), equation.ty.clone())).collect();
    candidate.signature.params = candidate
        .signature
        .params
        .iter()
        .map(|ty| substitute_associated_equations(&substitute_type_vars(ty, &substitution), subject, &equations))
        .collect();
    candidate.signature.return_type =
        substitute_associated_equations(&substitute_type_vars(&candidate.signature.return_type, &substitution), subject, &equations);
    candidate.generic_binders.retain(|binder| binder.owner != trait_id);
    // 未标注 self 仍是声明接收者槽，bound 已提供精确类型。
    if candidate.param_specs.first().is_some_and(|p| p.name.name.as_str() == "self")
        && matches!(candidate.signature.params.first(), Some(ValkyrieType::AutoType | ValkyrieType::SelfType))
    {
        candidate.signature.params[0] = subject.clone();
    }
    Some(candidate)
}

/// 未声明 bound 的泛型接收者：把成员实例化为 trait 方法合同本身。
pub(crate) fn instantiate_trait_method_candidate(candidate: &OverloadCandidate, subject: &ValkyrieType) -> Option<OverloadCandidate> {
    let trait_id = candidate.trait_owner?;
    let mut candidate = substitute_trait_self(candidate, subject);
    if candidate.generic_binders.iter().any(|binder| binder.owner == trait_id) {
        return None;
    }
    if candidate.param_specs.first().is_some_and(|p| p.name.name.as_str() == "self")
        && matches!(candidate.signature.params.first(), Some(ValkyrieType::AutoType | ValkyrieType::SelfType))
    {
        candidate.signature.params[0] = subject.clone();
    }
    if let Some(param) = candidate.param_specs.first_mut() {
        if matches!(param.ty, ValkyrieType::AutoType | ValkyrieType::SelfType) {
            param.ty = subject.clone();
        }
    }
    Some(candidate)
}

/// 把候选中的 SelfType 全部替换为 owner_type，并处理 where 约束中的类型。
pub(crate) fn substitute_trait_self(candidate: &OverloadCandidate, owner_type: &ValkyrieType) -> OverloadCandidate {
    let mut candidate = candidate.clone();
    candidate.signature.params = candidate.signature.params.iter().map(|ty| substitute_type_self(ty, owner_type)).collect();
    candidate.signature.return_type = substitute_type_self(&candidate.signature.return_type, owner_type);
    candidate.where_constraints = candidate
        .where_constraints
        .iter()
        .map(|constraint| HirWhereConstraint {
            target: substitute_type_self(&constraint.target, owner_type),
            bounds: constraint
                .bounds
                .iter()
                .map(|bound| HirTraitBound {
                    declaration: bound.declaration,
                    trait_path: bound.trait_path.clone(),
                    type_arguments: bound.type_arguments.iter().map(|ty| substitute_type_self(ty, owner_type)).collect(),
                    associated_types: bound
                        .associated_types
                        .iter()
                        .map(|equation| HirAssociatedTypeBinding {
                            name: equation.name.clone(),
                            ty: substitute_type_self(&equation.ty, owner_type),
                        })
                        .collect(),
                })
                .collect(),
            span: constraint.span.clone(),
        })
        .collect();
    candidate
}

pub(crate) fn unify_call_type_binders(
    expected: &ValkyrieType,
    actual: &ValkyrieType,
    binders: &std::collections::BTreeSet<GenericBinderId>,
    substitutions: &mut BTreeMap<GenericBinderId, ValkyrieType>,
) -> Option<()> {
    match expected {
        ValkyrieType::Generic(generic) => {
            let binder = generic.binder?;
            if !binders.contains(&binder) {
                return (expected == actual).then_some(());
            }
            match substitutions.get(&binder) {
                Some(bound) if bound != actual => None,
                Some(_) => Some(()),
                None => {
                    substitutions.insert(binder, actual.clone());
                    Some(())
                }
            }
        }
        ValkyrieType::Array(expected_element) => {
            let ValkyrieType::Array(actual_element) = actual
            else {
                return None;
            };
            unify_call_type_binders(expected_element, actual_element, binders, substitutions)
        }
        ValkyrieType::Apply(expected_base, expected_args) => {
            let ValkyrieType::Apply(actual_base, actual_args) = actual
            else {
                return None;
            };
            if expected_args.len() != actual_args.len() {
                return None;
            }
            unify_call_type_binders(expected_base, actual_base, binders, substitutions)?;
            for (expected, actual) in expected_args.iter().zip(actual_args) {
                unify_call_type_binders(expected, actual, binders, substitutions)?;
            }
            Some(())
        }
        ValkyrieType::Nominal { declaration: expected_decl, arguments: expected_args } => {
            let ValkyrieType::Nominal { declaration: actual_decl, arguments: actual_args } = actual
            else {
                return None;
            };
            if expected_decl != actual_decl || expected_args.len() != actual_args.len() {
                return None;
            }
            for (expected, actual) in expected_args.iter().zip(actual_args) {
                unify_call_type_binders(expected, actual, binders, substitutions)?;
            }
            Some(())
        }
        _ => Some(()),
    }
}

pub(crate) fn specialize_call_literals(
    args: &[crate::compiler::types::hir::HirCallArgument],
    actual: &[Option<ValkyrieType>],
    expected: &[ValkyrieType],
    type_relations: &TypeRelationContext,
) -> Vec<Option<ValkyrieType>> {
    let exprs = args.iter().map(|arg| arg.value.clone()).collect::<Vec<_>>();
    specialize_call_literal_exprs(&exprs, actual, expected, type_relations)
}

pub(crate) fn specialize_call_literal_exprs(
    exprs: &[crate::compiler::types::hir::HirExpr],
    actual: &[Option<ValkyrieType>],
    expected: &[ValkyrieType],
    type_relations: &TypeRelationContext,
) -> Vec<Option<ValkyrieType>> {
    exprs
        .iter()
        .zip(actual.iter().zip(expected.iter()))
        .map(|(expr, (actual, expected))| actual.as_ref().map(|actual| specialize_literal_expr_type(expr, actual, expected, type_relations)))
        .collect()
}

/// 单个实参按形参类型做字面量特化；形参必须是已代入泛型后的具体类型。
pub(crate) fn specialize_literal_expr_type(
    expr: &crate::compiler::types::hir::HirExpr,
    actual: &ValkyrieType,
    expected: &ValkyrieType,
    type_relations: &TypeRelationContext,
) -> ValkyrieType {
    use crate::compiler::types::hir::HirExprKind;
    match &expr.kind {
        // Integer literals are representation-polymorphic: bind to the formal
        // numeric type (`usize`, `i32`, …), same as constructor literals.
        // `Nominal{ItemId}` 的 primitive 也必须认，不能只认 `Named("usize")`。
        HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::Integer64(_)) if type_relations.is_numeric_type(expected) => {
            expected.clone()
        }
        // Float literals only bind to float formals: `1.0` is not a representation of any
        // integer, so it must not be silently accepted for `i32`/`usize`. Only `f32`/`f64`
        // accept Float64 literals in source (e.g. `1000000.0`).
        HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::Float64(_)) if type_relations.is_float_type(expected) => expected.clone(),
        // String literals infer as `Utf8` but core APIs take `text`.
        HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::String(_))
            if crate::compiler::hir::overload::types::is_text_type(expected) =>
        {
            expected.clone()
        }
        // 空数组字面量没有自足类型，只能由形式参数类型提供上下文；保留 `None`
        // 交给统一阶段做形状校验，不得在这里猜元素类型。
        _ => actual.clone(),
    }
}

/// 成员调用的实参上下文类型（不含接收者槽）。
pub(crate) fn member_call_argument_contexts(
    callee: &crate::compiler::types::hir::HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Vec<Option<ValkyrieType>> {
    use super::{call_resolver::candidate_has_receiver_parameter, symbol_match::candidate_owned_method_matches};
    use crate::compiler::types::hir::HirExprKind;

    // 成员调用有两种等价表示：`FieldAccess` callee，以及解析器保留的扁平
    // `Path([receiver, ..fields, method])`；后者根必须是本地绑定。
    let (receiver_type, field) = match &callee.kind {
        HirExprKind::FieldAccess { object, field, .. } => {
            let receiver_type = crate::compiler::hir::overload::types::infer_expr_type(object, locals)
                .or_else(|| crate::compiler::hir::overload::types::infer_scrutinee_type(object, locals, struct_fields, singleton_names));
            (receiver_type, field.clone())
        }
        HirExprKind::Path(path) if path.parts().len() >= 2 && locals.contains_key(path.parts()[0].as_str()) => {
            let mut receiver_type = locals.get(path.parts()[0].as_str()).cloned();
            for field_name in &path.parts()[1..path.parts().len() - 1] {
                receiver_type = receiver_type
                    .and_then(|ty| crate::compiler::hir::overload::types::lookup_struct_field_type(&ty, field_name, struct_fields));
            }
            (receiver_type, path.parts().last().cloned().unwrap_or_default())
        }
        _ => return Vec::new(),
    };
    let Some(receiver_type) = receiver_type
    else {
        return Vec::new();
    };
    let mut contexts = Vec::new();
    let mut conflicted = Vec::new();
    let mut applicable = false;
    for candidate in candidates
        .iter()
        .filter(|candidate| matches!(candidate.domain, crate::compiler::hir::overload::diagnostics::OverloadDomain::Function))
        .filter(|candidate| candidate_has_receiver_parameter(candidate))
        .filter(|candidate| candidate_owned_method_matches(candidate, &field))
    {
        let Some(owner) = candidate.owner.as_ref()
        else {
            continue;
        };
        // `Nominal{ItemId}` 没有表面名，必须按 declaration 判定接收者是否属于候选 owner；
        // 只有 `Named`/`Array` 这类仍带表面名的形态才按短名比较。
        let owner_declaration = match type_relations.nominal_type_for_name(owner) {
            Some(ValkyrieType::Nominal { declaration, .. }) => Some(declaration),
            _ => None,
        };
        let belongs = match (&receiver_type, owner_declaration) {
            (ValkyrieType::Nominal { declaration, .. }, Some(owner_declaration)) => *declaration == owner_declaration,
            _ => super::r#trait::impl_nominal_type_name(&receiver_type) == Some(owner.as_str()),
        };
        if !belongs {
            continue;
        }
        let Some(self_param) = candidate.signature.params.first()
        else {
            continue;
        };
        let mut substitutions = BTreeMap::new();
        if unify_call_type_binders(self_param, &receiver_type, &candidate.generic_binders, &mut substitutions).is_none() {
            continue;
        }
        applicable = true;
        for (index, ty) in candidate.signature.params.iter().enumerate().skip(1) {
            let context = substitute_type_vars(ty, &substitutions);
            let slot = index - 1;
            if contexts.len() <= slot {
                contexts.resize(slot + 1, None);
                conflicted.resize(slot + 1, false);
            }
            if conflicted[slot] {
                continue;
            }
            match &contexts[slot] {
                None => contexts[slot] = Some(context),
                Some(existing) if existing != &context => {
                    contexts[slot] = None;
                    conflicted[slot] = true;
                }
                Some(_) => {}
            }
        }
    }
    if !applicable {
        return Vec::new();
    }
    contexts
}

pub(crate) fn try_resolve_qualified_free_function_substitute_vars_only() {
    // no-op placeholder
}

pub(crate) fn extract_singleton_type_name(
    expr: &crate::compiler::types::hir::HirExpr,
    locals: &BTreeMap<String, ValkyrieType>,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<Identifier> {
    use crate::compiler::types::hir::HirExprKind;
    match &expr.kind {
        HirExprKind::Variable(identifier) if singleton_names.contains(&identifier.name) => Some(identifier.name.clone()),
        HirExprKind::Variable(identifier) => match locals.get(identifier.name.as_str())? {
            ValkyrieType::Named(name) if singleton_names.contains(name) => Some(name.clone()),
            _ => None,
        },
        HirExprKind::Path(path) if path.parts().len() == 1 && singleton_names.contains(&path.parts()[0]) => Some(path.parts()[0].clone()),
        _ => None,
    }
}

/// 由声明 owner 的泛型实参位置代入 receiver 的具体参数（用于 singleton method 实参类型特化）。
pub(crate) fn substitute_type_parameters(ty: &ValkyrieType, receiver: &ValkyrieType, actual: &ValkyrieType) -> ValkyrieType {
    // Apply 与 Nominal 的类型参数槽同位：按槽位把声明 binder 代入实际参数。
    // 不得再要求表面名为 Option/Result；declaration identity 已由候选 sum_owner 携带。
    let (receiver_args, actual_args) = match (receiver, actual) {
        (ValkyrieType::Apply(_, receiver_args), ValkyrieType::Apply(_, actual_args)) => (receiver_args.as_slice(), actual_args.as_slice()),
        (
            ValkyrieType::Nominal { declaration: receiver_decl, arguments: receiver_args },
            ValkyrieType::Nominal { declaration: actual_decl, arguments: actual_args },
        ) if receiver_decl == actual_decl => (receiver_args.as_slice(), actual_args.as_slice()),
        _ => return ty.clone(),
    };
    if receiver_args.len() != actual_args.len() {
        return ty.clone();
    }
    let substitutions = receiver_args
        .iter()
        .zip(actual_args.iter())
        .filter_map(|(param, arg)| match param {
            ValkyrieType::Generic(generic) => generic.binder.map(|binder| (binder, arg.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    substitute_type_vars(ty, &substitutions)
}

pub(crate) fn substitute_type_vars(ty: &ValkyrieType, substitutions: &BTreeMap<GenericBinderId, ValkyrieType>) -> ValkyrieType {
    match ty {
        ValkyrieType::Generic(generic) => generic.binder.and_then(|binder| substitutions.get(&binder)).cloned().unwrap_or_else(|| ty.clone()),
        ValkyrieType::Nominal { declaration, arguments } => ValkyrieType::Nominal {
            declaration: *declaration,
            arguments: arguments.iter().map(|arg| substitute_type_vars(arg, substitutions)).collect(),
        },
        ValkyrieType::Apply(base, args) => ValkyrieType::Apply(
            Box::new(substitute_type_vars(base, substitutions)),
            args.iter().map(|arg| substitute_type_vars(arg, substitutions)).collect(),
        ),
        ValkyrieType::Array(inner) => ValkyrieType::Array(Box::new(substitute_type_vars(inner, substitutions))),
        ValkyrieType::FixedArray { element, length } => {
            ValkyrieType::FixedArray { element: Box::new(substitute_type_vars(element, substitutions)), length: *length }
        }
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(items.iter().map(|item| substitute_type_vars(item, substitutions)).collect()),
        ValkyrieType::Union(items) => ValkyrieType::Union(items.iter().map(|item| substitute_type_vars(item, substitutions)).collect()),
        ValkyrieType::Intersection(items) => {
            ValkyrieType::Intersection(items.iter().map(|item| substitute_type_vars(item, substitutions)).collect())
        }
        ValkyrieType::Nullable(inner) => ValkyrieType::Nullable(Box::new(substitute_type_vars(inner, substitutions))),
        ValkyrieType::Function(function) => ValkyrieType::Function(Box::new(crate::compiler::types::hir::FunctionType {
            params: function.params.iter().map(|param| substitute_type_vars(param, substitutions)).collect(),
            return_type: substitute_type_vars(&function.return_type, substitutions),
        })),
        ValkyrieType::Associated(associated) => ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
            base: substitute_type_vars(&associated.base, substitutions),
            name: associated.name.clone(),
            type_arguments: associated.type_arguments.iter().map(|ty| substitute_type_vars(ty, substitutions)).collect(),
        })),
        other => other.clone(),
    }
}
