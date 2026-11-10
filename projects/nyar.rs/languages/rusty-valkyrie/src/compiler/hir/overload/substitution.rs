//! 类型代换与泛型统一。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::type_relation::{ParameterMatchResult, TypeRelationContext},
    mir::AggregateFieldIndex,
    types::{
        Identifier, NamePath,
        hir::{
            AssociatedType, FunctionType, GenericBinderId, HirAssociatedTypeBinding, HirCallArgument, HirLiteral, HirParam, HirTraitBound,
            HirTupleField, HirWhereConstraint, RowMethodType, RowType, TraitObject, TypeLambda, ValkyrieType,
        },
    },
};

use super::diagnostics::{OverloadCandidate, OverloadDomain};
use super::r#trait::impl_nominal_type_name;
use super::symbol_match::aggregate_constructor_matches_name;

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

/// 用调用点期望类型收敛已解析调用中仍开放的 binder（典型是 `collect<I, T, C> -> C`
/// 里只出现在返回形状的 `C`）。
///
/// 只消费**完全具体**的期望类型：含开放 binder 的上下文（例如无标注 `let` 继承的
/// 外层返回类型）不是本次调用的求解证据，绑定它会把两个不同作用域的 binder 混为
/// 一谈。收敛结果写回合同签名与 binder 解，供单态化与后续校验直接消费。
pub(crate) fn converge_call_with_context(
    call: &mut crate::compiler::types::hir::HirResolvedCall,
    contextual_type: &ValkyrieType,
    type_relations: &TypeRelationContext,
) {
    if has_unresolved_generic_arguments(contextual_type) || !has_unresolved_generic_arguments(&call.return_type) {
        return;
    }
    let binders = owner_type_generic_binders(&call.return_type);
    if binders.is_empty() {
        return;
    }
    let mut substitutions = BTreeMap::new();
    if unify_call_type_binders(&call.return_type, contextual_type, &binders, &mut substitutions, type_relations).is_none()
        || substitutions.is_empty()
    {
        return;
    }
    call.return_type = substitute_type_vars(&call.return_type, &substitutions);
    for parameter in &mut call.parameter_types {
        *parameter = substitute_type_vars(parameter, &substitutions);
    }
    for (binder, solution) in substitutions {
        call.associated_binder_solutions.entry(binder).or_insert(solution);
    }
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

/// 对复合类型的直接子类型施加 `map` 并按同一构造重建。
///
/// 返回 `None` 表示该变体不携带任何子类型（叶子）。本函数对 [`ValkyrieType`]
/// **全变体穷尽匹配**：新增变体必须在此明确它是叶子还是复合载体，否则
/// non-exhaustive 编译错误会直接拦住，从结构上防止「新增复合载体漏递归」。
/// 所有对类型做 Self/owner/泛型/关联类型代入的入口都必须经由本函数递归，
/// 不得各自手写一份只覆盖部分变体的 match。
fn rebuild_children(
    ty: &ValkyrieType,
    map: &mut impl FnMut(&ValkyrieType) -> ValkyrieType,
) -> Option<ValkyrieType> {
    fn map_all(map: &mut impl FnMut(&ValkyrieType) -> ValkyrieType, items: &[ValkyrieType]) -> Vec<ValkyrieType> {
        items.iter().map(|item| map(item)).collect()
    }

    Some(match ty {
        ValkyrieType::Nominal { declaration, arguments, primitive_identity } => ValkyrieType::Nominal {
            declaration: *declaration,
            arguments: map_all(map, arguments),
            primitive_identity: primitive_identity.clone(),
        },
        ValkyrieType::Apply(base, arguments) => ValkyrieType::Apply(Box::new(map(base)), map_all(map, arguments)),
        ValkyrieType::Array(element) => ValkyrieType::Array(Box::new(map(element))),
        ValkyrieType::FixedArray { element, length } => {
            ValkyrieType::FixedArray { element: Box::new(map(element)), length: *length }
        }
        ValkyrieType::Function(function) => ValkyrieType::Function(Box::new(FunctionType {
            params: map_all(map, &function.params),
            return_type: map(&function.return_type),
        })),
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(map_all(map, items)),
        ValkyrieType::LabeledTuple(fields) => {
            ValkyrieType::LabeledTuple(fields.iter().map(|field| HirTupleField { name: field.name.clone(), ty: map(&field.ty) }).collect())
        }
        ValkyrieType::Union(items) => ValkyrieType::Union(map_all(map, items)),
        ValkyrieType::Intersection(items) => ValkyrieType::Intersection(map_all(map, items)),
        ValkyrieType::Nullable(inner) => ValkyrieType::Nullable(Box::new(map(inner))),
        ValkyrieType::Row(row) => ValkyrieType::Row(RowType {
            methods: row
                .methods
                .iter()
                .map(|method| RowMethodType {
                    name: method.name.clone(),
                    params: map_all(map, &method.params),
                    return_type: map(&method.return_type),
                })
                .collect(),
        }),
        ValkyrieType::TypeLambda(lambda) => {
            ValkyrieType::TypeLambda(Box::new(TypeLambda { params: lambda.params.clone(), body: map(&lambda.body) }))
        }
        ValkyrieType::TraitObject(object) => ValkyrieType::TraitObject(TraitObject {
            trait_path: object.trait_path.clone(),
            type_arguments: map_all(map, &object.type_arguments),
        }),
        ValkyrieType::Associated(associated) => ValkyrieType::Associated(Box::new(AssociatedType {
            base: map(&associated.base),
            name: associated.name.clone(),
            type_arguments: map_all(map, &associated.type_arguments),
        })),
        // 叶子变体：不自带任何子类型（`Generic` 的 binder 由泛型代换单独处理）。
        ValkyrieType::Void
        | ValkyrieType::Unit
        | ValkyrieType::Boolean
        | ValkyrieType::Integer8 { .. }
        | ValkyrieType::Integer16 { .. }
        | ValkyrieType::Integer32 { .. }
        | ValkyrieType::Integer64 { .. }
        | ValkyrieType::Integer128 { .. }
        | ValkyrieType::Float32
        | ValkyrieType::Float64
        | ValkyrieType::Character
        | ValkyrieType::Utf8
        | ValkyrieType::Utf16
        | ValkyrieType::Named(_)
        | ValkyrieType::QualifiedPath { .. }
        | ValkyrieType::Generic(_)
        | ValkyrieType::AutoType
        | ValkyrieType::SelfType => return None,
    })
}

/// 把类型中的 `SelfType` 替换为给定主体；外部测试入口。
#[doc(hidden)]
pub fn substitute_type_self(ty: &ValkyrieType, replacement: &ValkyrieType) -> ValkyrieType {
    if matches!(ty, ValkyrieType::SelfType) {
        return replacement.clone();
    }
    rebuild_children(ty, &mut |child| substitute_type_self(child, replacement)).unwrap_or_else(|| ty.clone())
}

/// Replace `Self` with the imply/class owner so instance dispatch is nominal.
pub(crate) fn substitute_self_type(ty: &ValkyrieType, owner: Option<&Identifier>) -> ValkyrieType {
    if matches!(ty, ValkyrieType::r#SelfType) {
        return match owner {
            Some(owner) => ValkyrieType::Named(owner.clone()),
            None => ty.clone(),
        };
    }
    rebuild_children(ty, &mut |child| substitute_self_type(child, owner)).unwrap_or_else(|| ty.clone())
}

/// 按关联类型等式替换 `Associated` 形状；外部测试入口。
#[doc(hidden)]
pub fn substitute_associated_equations(
    ty: &ValkyrieType,
    constrained_type: &ValkyrieType,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) -> ValkyrieType {
    if let ValkyrieType::Associated(associated) = ty {
        if &associated.base == constrained_type {
            if let Some(equation) = equations.get(&associated.name) {
                return equation.clone();
            }
        }
    }
    rebuild_children(ty, &mut |child| substitute_associated_equations(child, constrained_type, equations))
        .unwrap_or_else(|| ty.clone())
}

/// 按外部提供的见证逐层重写 `Associated` 形状；见证返回 `None` 的投影保持原样。
///
/// 与 [`substitute_associated_equations`] 的区别是解来源不预先给定等式表：调用方
/// 可以在回调里按投影基类型查 trait 实现的关联类型见证，投影基类型仍是开放 binder
/// 时由回调拒绝解析，保持 fail-closed。
pub(crate) fn rewrite_associated_projections(
    ty: &ValkyrieType,
    resolve: &mut impl FnMut(&AssociatedType) -> Option<ValkyrieType>,
) -> ValkyrieType {
    if let ValkyrieType::Associated(associated) = ty {
        if let Some(resolved) = resolve(associated) {
            return rewrite_associated_projections(&resolved, resolve);
        }
    }
    rebuild_children(ty, &mut |child| rewrite_associated_projections(child, resolve)).unwrap_or_else(|| ty.clone())
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
    type_relations: &TypeRelationContext,
) -> Option<()> {
    // 实参类型未知（未标注形参等 `AutoType`）时没有可绑定的类型形参；与
    // `trivially_compatible` 的 `actual AutoType` 规则同源，这里不得提前否定候选。
    if matches!(actual, ValkyrieType::AutoType) {
        return Some(());
    }
    if type_relations.text_carriers_equivalent(actual, expected) {
        return Some(());
    }
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
            unify_call_type_binders(expected_element, actual_element, binders, substitutions, type_relations)
        }
        ValkyrieType::Apply(expected_base, expected_args) => {
            if let ValkyrieType::Apply(actual_base, actual_args) = actual {
                if expected_args.len() != actual_args.len() {
                    return None;
                }
                unify_call_type_binders(expected_base, actual_base, binders, substitutions, type_relations)?;
                for (expected, actual) in expected_args.iter().zip(actual_args) {
                    unify_call_type_binders(expected, actual, binders, substitutions, type_relations)?;
                }
                return Some(());
            }
            // 跨包导出的 impl owner 丢失 `constructor_declaration`，落成
            // `Apply(Named("X"), [Generic])`；接收者仍冻结为 `Nominal{X}`。两者是同一
            // 名义类型，必须按声明身份把泛型实参逐个绑定，不得用兜底分支放行。
            if let ValkyrieType::Nominal { declaration, arguments: actual_args, .. } = actual
                && let Some(base_name) = impl_nominal_type_name(expected_base)
            {
                let base_name = Identifier::new(base_name);
                if !type_relations.owner_name_matches_declaration(&base_name, *declaration) || expected_args.len() != actual_args.len() {
                    return None;
                }
                for (expected, actual) in expected_args.iter().zip(actual_args) {
                    unify_call_type_binders(expected, actual, binders, substitutions, type_relations)?;
                }
                return Some(());
            }
            None
        }
        ValkyrieType::Nominal { declaration: expected_decl, arguments: expected_args, .. } => {
            if let ValkyrieType::Nominal { declaration: actual_decl, arguments: actual_args, .. } = actual {
                if expected_decl != actual_decl || expected_args.len() != actual_args.len() {
                    return None;
                }
                for (expected, actual) in expected_args.iter().zip(actual_args) {
                    unify_call_type_binders(expected, actual, binders, substitutions, type_relations)?;
                }
                return Some(());
            }
            if let ValkyrieType::Apply(actual_base, actual_args) = actual {
                let Some(actual_name) = impl_nominal_type_name(actual_base).map(Identifier::new)
                else {
                    return None;
                };
                if !type_relations.owner_name_matches_declaration(&actual_name, *expected_decl) || expected_args.len() != actual_args.len() {
                    return None;
                }
                for (expected, actual) in expected_args.iter().zip(actual_args) {
                    unify_call_type_binders(expected, actual, binders, substitutions, type_relations)?;
                }
                return Some(());
            }
            None
        }
        // 函数类型逐项统一：形参 `micro(T, T) -> i32` 这类含 binder 的期望签名，
        // 必须由具名函数作值的冻结签名把 T 绑定出来，禁止宽松放行后留给后端猜。
        ValkyrieType::Function(expected_function) => {
            let ValkyrieType::Function(actual_function) = actual
            else {
                return None;
            };
            if expected_function.params.len() != actual_function.params.len() {
                return None;
            }
            for (expected, actual) in expected_function.params.iter().zip(&actual_function.params) {
                unify_call_type_binders(expected, actual, binders, substitutions, type_relations)?;
            }
            unify_call_type_binders(&expected_function.return_type, &actual_function.return_type, binders, substitutions, type_relations)
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
        // Character literals are representation-polymorphic like integer literals: a `char`
        // formal binds directly, a numeric formal binds the code point value, so
        // `code_point == ' '` (u32 vs char literal) needs no `as` cast at the call site.
        HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::Character(_)) if type_relations.is_character_literal_formal(expected) => {
            expected.clone()
        }
        // Byte literals bind to numeric formals (`b' '` in a `u32` comparison), same rule
        // as integer literals; the `u8` value range was validated during HIR lowering.
        HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::Byte(_)) if type_relations.is_numeric_type(expected) => expected.clone(),
        // Float literals only bind to float formals: `1.0` is not a representation of any
        // integer, so it must not be silently accepted for `i32`/`usize`. Only `f32`/`f64`
        // accept Float64 literals in source (e.g. `1000000.0`).
        HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::Float64(_)) if type_relations.is_float_type(expected) => expected.clone(),
        // String literals infer as `Utf8` but core APIs take `text`/`utf8`
        // (possibly behind an alias resolved to `Nominal{ItemId}`).
        HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::String(_)) if type_relations.is_text_formal(expected) => expected.clone(),
        // 数组字面量的元素与调用实参同属「接收位置」：字面量元素按形式参数的元素类型
        // 逐项特化（`[104, 101]` 绑定到 `[u8]`）。只有全部元素都能特化到同一元素类型
        // 时才整体改写；任一元素（变量、浮点字面量等）特化不成即保留原推断，
        // 交给统一阶段失败，不得在这里猜元素类型。
        HirExprKind::ArrayLiteral { items } if !items.is_empty() && matches!(expected, ValkyrieType::Array(_)) => {
            let ValkyrieType::Array(expected_item) = expected
            else {
                unreachable!()
            };
            let empty_locals = BTreeMap::new();
            let items_specialized = items
                .iter()
                .map(|item| {
                    crate::compiler::hir::overload::types::infer_expr_type(item, &empty_locals)
                        .map(|item_actual| specialize_literal_expr_type(item, &item_actual, expected_item, type_relations))
                })
                .collect::<Option<Vec<_>>>();
            match items_specialized {
                Some(types) if types.iter().all(|ty| ty == expected_item.as_ref()) => expected.clone(),
                _ => actual.clone(),
            }
        }
        // 元组字面量没有标签语法，标签只能由形式参数提供。只有形式参数是
        // `LabeledTuple`、实参确实是元组字面量、且逐位置类型（嵌套字面量先按
        // 对应字段特化）完全一致时，才把整个字面量精化为该带标签类型。
        // 位置类型不等或形式参数不是 `LabeledTuple` 时保留原推断，交给统一
        // 阶段失败，不得把 `Tuple` 与 `LabeledTuple` 全局视为兼容而擦掉标签。
        HirExprKind::TupleLiteral { items } => {
            let ValkyrieType::LabeledTuple(fields) = expected else {
                return actual.clone();
            };
            let ValkyrieType::Tuple(actual_items) = actual else {
                return actual.clone();
            };
            if items.len() != fields.len() || actual_items.len() != fields.len() {
                return actual.clone();
            }
            let specialized = items
                .iter()
                .zip(actual_items.iter())
                .zip(fields.iter())
                .map(|((item, item_actual), field)| specialize_literal_expr_type(item, item_actual, &field.ty, type_relations))
                .collect::<Vec<_>>();
            if specialized.iter().zip(fields.iter()).all(|(ty, field)| ty == &field.ty) {
                expected.clone()
            } else {
                actual.clone()
            }
        }
        // 空数组字面量没有自足类型，只能由形式参数类型提供上下文；保留 `None`
        // 交给统一阶段做形状校验，不得在这里猜元素类型。
        _ => actual.clone(),
    }
}

/// 成员调用的实参上下文类型（不含接收者槽）。
pub(crate) fn member_call_argument_contexts(
    callee: &crate::compiler::types::hir::HirExpr,
    args: &[HirCallArgument],
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
    //
    // 运算符调用（`a == b` / `-a` / `a[b]`）是第三种形态：callee 为单段运算符成员名，
    // 接收者作为第一个实参出现。`receiver_in_args` 记录该形态，后续按实参位对齐槽位。
    let (receiver_type, field, receiver_in_args) = match &callee.kind {
        HirExprKind::FieldAccess { object, field, .. } => {
            let receiver_type = crate::compiler::hir::overload::types::infer_expr_type(object, locals)
                .or_else(|| crate::compiler::hir::overload::types::infer_scrutinee_type(object, locals, struct_fields, singleton_names));
            (receiver_type, field.clone(), false)
        }
        HirExprKind::Path(path) if path.parts().len() >= 2 && locals.contains_key(path.parts()[0].as_str()) => {
            let mut receiver_type = locals.get(path.parts()[0].as_str()).cloned();
            for field_name in &path.parts()[1..path.parts().len() - 1] {
                receiver_type = receiver_type
                    .and_then(|ty| crate::compiler::hir::overload::types::lookup_struct_field_type(&ty, field_name, struct_fields));
            }
            (receiver_type, path.parts().last().cloned().unwrap_or_default(), false)
        }
        HirExprKind::Path(path)
            if path.parts().len() == 1
                && crate::compiler::semantic::operator_members::is_operator_member_name(path.parts()[0].as_str()) =>
        {
            // 运算符接收者没有自足类型时不供给上下文：候选跨数值宽度时槽位冲突，
            // 交给统一阶段结构化失败，不在这里猜。
            let Some(receiver) = args.first()
            else {
                return Vec::new()
            };
            // 裸数值字面量接收者没有自足宽度：它的类型必须由调用点期望类型决定。
            // 用默认宽度推断出的接收者类型去回填实参槽，会把它升格成权威上下文，
            // 反向锁死外层候选宽度（`let x: i64 = 7 / y` 会退化成 i32）。
            if matches!(
                &receiver.value.kind,
                HirExprKind::Literal(HirLiteral::Integer64(_) | HirLiteral::Float64(_))
            ) {
                return Vec::new();
            }
            let receiver_type = crate::compiler::hir::overload::types::infer_expr_type(&receiver.value, locals)
                .or_else(|| crate::compiler::hir::overload::types::infer_scrutinee_type(&receiver.value, locals, struct_fields, singleton_names));
            (receiver_type, path.parts()[0].clone(), true)
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
        if unify_call_type_binders(self_param, &receiver_type, &candidate.generic_binders, &mut substitutions, type_relations).is_none() {
            continue;
        }
        applicable = true;
        // 运算符调用的实参已包含接收者（`args[0]`），参数位与实参位一一对应；
        // 普通成员调用的实参不含接收者，参数位相对实参位右移一格。
        let (skip, offset): (usize, usize) = if receiver_in_args { (0, 0) } else { (1, 1) };
        for (index, ty) in candidate.signature.params.iter().enumerate().skip(skip) {
            let context = substitute_type_vars(ty, &substitutions);
            let slot = index - offset;
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

/// 自由函数调用的实参上下文类型（裸名 callee，不含接收者槽）。
///
/// 位置实参逐槽期望类型的统一来源：callee 为名字或构造路径且不是局部绑定或
/// singleton 时，匹配同名自由函数候选与已冻结构造候选的形参槽；多个候选在同一
/// 槽给出不同类型时该槽置 `None`（冲突不猜）。
///
/// 槽位类型必须按调用点期望类型代入候选 binder：`Some(Some(5))` 的外层载荷槽
/// 只有代入后才是 `Option<i64>`，否则内层拿到的是开放 binder，字面量退化成默认
/// 宽度（`Option<i32>`）。代入只消费已冻结的候选签名与调用点上下文，不猜名字。
pub(crate) fn free_call_argument_contexts(
    callee: &crate::compiler::types::hir::HirExpr,
    candidates: &[OverloadCandidate],
    type_relations: &TypeRelationContext,
    locals: &BTreeMap<String, ValkyrieType>,
    singleton_names: &std::collections::BTreeSet<Identifier>,
    contextual_type: Option<&ValkyrieType>,
) -> Vec<Option<ValkyrieType>> {
    use crate::compiler::types::hir::HirExprKind;

    // 位置实参的期望类型来自**被调可调用声明**的形参槽，与调用语法形态无关：
    // 自由函数按未限定短名对齐，构造器（含变体）按已冻结的构造路径对齐。
    // 只收自由函数会让 `Some(Some(5))` 的内层实参失去载荷槽类型，退化成按
    // 字面量默认宽度推断（`Option<i32>`）。
    let free_name = match &callee.kind {
        HirExprKind::Variable(identifier) => Some(identifier.name.clone()),
        HirExprKind::Path(path) if path.parts().len() == 1 => Some(path.parts()[0].clone()),
        _ => None,
    };
    let constructor_path = match &callee.kind {
        HirExprKind::Variable(identifier) => Some(NamePath::new(vec![identifier.name.clone()])),
        HirExprKind::Path(path) if !path.parts().is_empty() => Some(path.clone()),
        _ => None,
    };
    let Some(callee_name) = free_name.clone().or_else(|| constructor_path.as_ref().and_then(|path| path.parts().last().cloned()))
    else {
        return Vec::new();
    };
    if locals.contains_key(callee_name.as_str()) || singleton_names.contains(&callee_name) {
        return Vec::new();
    }
    let enclosing_module = type_relations.enclosing_module();
    let mut contexts: Vec<Option<ValkyrieType>> = Vec::new();
    let mut conflicted: Vec<bool> = Vec::new();
    let mut applicable = false;
    for candidate in candidates.iter().filter(|candidate| {
        let free_function = matches!(candidate.domain, OverloadDomain::Function)
            && candidate.owner.is_none()
            && free_name.as_ref().is_some_and(|name| {
                // 候选身份必须与 `try_resolve_call` 的自由函数通道同口径，否则导入来的
                // 被调函数拿不到形参期望类型，裸变体实参会被外层返回类型误杀。
                super::symbol_match::symbol_matches_unqualified_free_call(&candidate.symbol, name, enclosing_module)
                    || super::symbol_match::symbol_matches_imported_dependency_export(candidate, name)
                    || type_relations.symbol_matches_import(&candidate.symbol, name)
                    || type_relations.symbol_matches_group_scope(&candidate.symbol, name)
            });
        let constructor = candidate.domain == OverloadDomain::Constructor
            && constructor_path.as_ref().is_some_and(|path| aggregate_constructor_matches_name(candidate, path));
        free_function || constructor
    }) {
        applicable = true;
        // 候选返回类型与调用点期望类型统一出的代入，用于把形参槽里的声明 binder
        // 落成该调用点的具体类型；统一失败或没有上下文时不代入，保持原槽位。
        let mut candidate_substitutions = BTreeMap::new();
        if let Some(contextual_type) = contextual_type {
            let mut substitutions = BTreeMap::new();
            if unify_call_type_binders(
                &candidate.signature.return_type,
                contextual_type,
                &candidate.generic_binders,
                &mut substitutions,
                type_relations,
            )
            .is_some()
            {
                candidate_substitutions = substitutions;
            }
        }
        for (slot, param) in candidate.signature.params.iter().enumerate() {
            let context = substitute_type_vars(param, &candidate_substitutions);
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
            ValkyrieType::Nominal { declaration: receiver_decl, arguments: receiver_args, .. },
            ValkyrieType::Nominal { declaration: actual_decl, arguments: actual_args, .. },
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
    if let ValkyrieType::Generic(generic) = ty {
        return generic.binder.and_then(|binder| substitutions.get(&binder)).cloned().unwrap_or_else(|| ty.clone());
    }
    rebuild_children(ty, &mut |child| substitute_type_vars(child, substitutions)).unwrap_or_else(|| ty.clone())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::compiler::types::hir::{GenericType, HirKind};

    fn item_id(index: u32) -> nyar_types::ItemId {
        nyar_types::ItemId::from_index(index).expect("测试 ItemId 必须可构造")
    }

    fn owner() -> ValkyrieType {
        ValkyrieType::Nominal { declaration: item_id(42), arguments: vec![ValkyrieType::Integer64 { signed: true }], primitive_identity: None }
    }

    fn associated(name: &str) -> ValkyrieType {
        ValkyrieType::Associated(Box::new(AssociatedType {
            base: ValkyrieType::SelfType,
            name: Identifier::new(name),
            type_arguments: vec![ValkyrieType::SelfType],
        }))
    }

    fn binder() -> GenericBinderId {
        GenericBinderId { owner: item_id(900), ordinal: 0 }
    }

    fn generic() -> ValkyrieType {
        ValkyrieType::Generic(GenericType { binder: Some(binder()), name: Identifier::new("T"), kind: HirKind::Type, bounds: vec![] })
    }

    fn assert_no_self_type(ty: &ValkyrieType) {
        assert!(!matches!(ty, ValkyrieType::SelfType), "SelfType 未被代入: {ty:?}");
        let _ = rebuild_children(ty, &mut |child| {
            assert_no_self_type(child);
            child.clone()
        });
    }

    /// 报告缺陷的最小形状：函数形参里 `Self::Item` 必须随 `Self` 一起代入。
    #[test]
    fn self_type_is_substituted_into_function_signature() {
        let ty = ValkyrieType::Function(Box::new(FunctionType { params: vec![associated("Item")], return_type: ValkyrieType::Boolean }));
        let substituted = substitute_type_self(&ty, &owner());
        let equations = BTreeMap::from([(Identifier::new("Item"), ValkyrieType::Integer64 { signed: true })]);
        assert_eq!(
            substitute_associated_equations(&substituted, &owner(), &equations),
            ValkyrieType::Function(Box::new(FunctionType {
                params: vec![ValkyrieType::Integer64 { signed: true }],
                return_type: ValkyrieType::Boolean,
            }))
        );
    }

    /// 所有复合载体都必须递归：这个清单与 `rebuild_children` 的复合分支一一对应。
    #[test]
    fn self_type_is_substituted_through_every_composite_carrier() {
        let carriers = vec![
            ValkyrieType::Nominal { declaration: item_id(7), arguments: vec![ValkyrieType::SelfType], primitive_identity: None },
            ValkyrieType::Apply(Box::new(ValkyrieType::Named(Identifier::new("F"))), vec![ValkyrieType::SelfType]),
            ValkyrieType::Array(Box::new(ValkyrieType::SelfType)),
            ValkyrieType::FixedArray { element: Box::new(ValkyrieType::SelfType), length: 3 },
            ValkyrieType::Function(Box::new(FunctionType { params: vec![ValkyrieType::SelfType], return_type: ValkyrieType::SelfType })),
            ValkyrieType::Tuple(vec![ValkyrieType::SelfType]),
            ValkyrieType::LabeledTuple(vec![HirTupleField { name: Identifier::new("field"), ty: ValkyrieType::SelfType }]),
            ValkyrieType::Union(vec![ValkyrieType::SelfType]),
            ValkyrieType::Intersection(vec![ValkyrieType::SelfType]),
            ValkyrieType::Nullable(Box::new(ValkyrieType::SelfType)),
            ValkyrieType::Row(RowType {
                methods: vec![RowMethodType {
                    name: Identifier::new("m"),
                    params: vec![ValkyrieType::SelfType],
                    return_type: ValkyrieType::SelfType,
                }],
            }),
            ValkyrieType::TypeLambda(Box::new(TypeLambda { params: vec![], body: ValkyrieType::SelfType })),
            ValkyrieType::TraitObject(TraitObject { trait_path: Identifier::new("Trait"), type_arguments: vec![ValkyrieType::SelfType] }),
            ValkyrieType::Associated(Box::new(AssociatedType {
                base: ValkyrieType::SelfType,
                name: Identifier::new("Item"),
                type_arguments: vec![ValkyrieType::SelfType],
            })),
        ];
        for carrier in carriers {
            assert_no_self_type(&substitute_type_self(&carrier, &owner()));
        }
    }

    /// owner 代入与泛型代换同样必须进入复合类型（FixedArray / Function / Intersection 曾各有漏项）。
    #[test]
    fn self_owner_and_type_vars_recurse_into_compound_carriers() {
        let function = ValkyrieType::Function(Box::new(FunctionType {
            params: vec![ValkyrieType::FixedArray { element: Box::new(ValkyrieType::r#SelfType), length: 2 }],
            return_type: ValkyrieType::Intersection(vec![ValkyrieType::r#SelfType, ValkyrieType::r#SelfType]),
        }));
        let expected_owner = ValkyrieType::Function(Box::new(FunctionType {
            params: vec![ValkyrieType::FixedArray {
                element: Box::new(ValkyrieType::Named(Identifier::new("Owner"))),
                length: 2,
            }],
            return_type: ValkyrieType::Intersection(vec![
                ValkyrieType::Named(Identifier::new("Owner")),
                ValkyrieType::Named(Identifier::new("Owner")),
            ]),
        }));
        assert_eq!(substitute_self_type(&function, Some(&Identifier::new("Owner"))), expected_owner);

        let generic_function = ValkyrieType::Function(Box::new(FunctionType {
            params: vec![ValkyrieType::FixedArray { element: Box::new(generic()), length: 2 }],
            return_type: ValkyrieType::Intersection(vec![generic(), generic()]),
        }));
        let substitutions = BTreeMap::from([(binder(), ValkyrieType::Utf8)]);
        assert_eq!(
            substitute_type_vars(&generic_function, &substitutions),
            ValkyrieType::Function(Box::new(FunctionType {
                params: vec![ValkyrieType::FixedArray { element: Box::new(ValkyrieType::Utf8), length: 2 }],
                return_type: ValkyrieType::Intersection(vec![ValkyrieType::Utf8, ValkyrieType::Utf8]),
            }))
        );
    }

    /// 测试侧唯一 primitive 声明来源（与 `tests/support/primitives.v` 同一份文本）。
    const TEST_PRIMITIVES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/primitives.v"));

    /// A/B 探针：trait 关联类型出现在值、`Option`、函数形参三种位置时，
    /// 按具体 owner 实例化的成员签名都必须能匹配。
    #[test]
    fn associated_type_in_value_option_and_function_positions_resolves() {
        let source = r#"
unite Option<T> { Some { value: T }, None }

trait Iterator {
    type Item;
    micro accept_item(self, value: Item) -> bool { return true }
    micro accept_opt(self, value: Option<Item>) -> bool { return true }
    micro accept_fn(self, pred: micro(Item) -> bool) -> bool { return true }
}

structure ArrayIterator { }

imply ArrayIterator: Iterator { type Item = i64; }

micro is_even(value: i64) -> bool { return true }

micro probe(iter: ArrayIterator) -> bool {
    let item: i64 = 2
    let maybe: Option<i64> = None
    return iter.accept_item(item) && iter.accept_opt(maybe) && iter.accept_fn(is_even)
}
"#;
        crate::ValkyrieCompiler::default()
            .compile_source(&format!("{source}\n{TEST_PRIMITIVES}"))
            .unwrap_or_else(|error| panic!("trait 关联类型在三种位置都必须匹配: {error}"));
    }

    fn filter_probe_source(filter_iterator_body: &str) -> String {
        format!(
            r#"
unite Option<T> {{ Some {{ value: T }}, None }}

trait Iterator {{
    type Item;
    micro has_next(self): bool
    micro next(mut self): Option<Item>

    micro filter(self, pred: micro(Item) -> bool) -> FilterIterator<Item, Self> {{
        return FilterIterator {{ _iter: self, _predicate: pred }}
    }}
}}

structure FilterIterator<T, I> {{
    _iter: I
    _predicate: micro(T) -> bool
}}

imply FilterIterator<T, I>: Iterator {{
    type Item = T;
{filter_iterator_body}
}}

structure ArrayIterator {{ _index: usize }}

imply ArrayIterator: Iterator {{
    type Item = i64;
    micro has_next(self): bool {{ return true }}
    micro next(mut self): Option<i64> {{ return None }}
}}

micro is_even(value: i64) -> bool {{ return true }}

micro probe(iter: ArrayIterator) -> bool {{
    iter.filter(is_even)
    return true
}}
"#
        )
    }

    /// 整程序管线（含基本类型声明，贴近 std 形态）里，trait 默认方法
    /// 以函数值谓词调用具体接收者时必须能解析；这也是 wasm 探针的源形状。
    #[test]
    fn filter_default_method_compiles_through_program_pipeline() {
        let source = r#"
namespace app;
trait Iterator {
    type Item;
    micro filter(self, pred: micro(Item) -> bool) -> bool { return true }
}
structure ArrayIterator { _flag: bool }
imply ArrayIterator: Iterator { type Item = bool; }
micro is_even(value: bool) -> bool { return true }
[export(name: "run")]
micro run() -> i32 {
    let iter: ArrayIterator = ArrayIterator { _flag: true }
    iter.filter(is_even)
    return 7
}
"#;
        let group = crate::compiler::hir::lowering::CompilerSourceGroup::from_source_files(
            "app",
            "app",
            vec![
                crate::compiler::hir::lowering::CompilerSourceFile {
                    path: "core_primitive.v".into(),
                    source: "[primitive(\"std::primitive::i32\")] structure i32 { }\n[primitive(\"std::primitive::i64\")] structure i64 { }".into(),
                },
                crate::compiler::hir::lowering::CompilerSourceFile { path: "main.v".into(), source: source.into() },
            ],
            Vec::new(),
        )
        .as_entry();
        crate::ValkyrieCompiler::default()
            .compile_source_groups_to_program(&[group])
            .unwrap_or_else(|error| panic!("legion 形态探针: {error}"));
    }

    /// `arr.into_iterator().filter(is_even)` 的最小形状：函数值谓词的关联类型
    /// 必须随 `Self` 代入后参与实参匹配。
    #[test]
    fn filter_on_concrete_iterator_resolves_with_function_predicate() {
        let source = filter_probe_source(
            "    micro has_next(self): bool { return true }\n    micro next(mut self): Option<T> { return None }",
        );
        crate::ValkyrieCompiler::default()
            .compile_source(&format!("{source}\n{TEST_PRIMITIVES}"))
            .unwrap_or_else(|error| panic!("filter 的谓词形参必须能匹配: {error}"));
    }

    /// 未标注形参在 HIR 里是 `AutoType`：候选统一不得因此否定候选
    /// （与 `TypeRelationContext::match_parameter` 的 `actual AutoType` 规则同源）。
    #[test]
    fn auto_type_actual_is_not_rejected_during_binder_unification() {
        let relations = TypeRelationContext::from_module(&crate::compiler::types::hir::HirModule::default());
        let expected = ValkyrieType::Nominal { declaration: item_id(40), arguments: vec![], primitive_identity: None };
        let binders = std::collections::BTreeSet::new();
        let mut substitutions = BTreeMap::new();
        assert_eq!(unify_call_type_binders(&expected, &ValkyrieType::AutoType, &binders, &mut substitutions, &relations), Some(()));
        // 未知实参不是「任意名义类型都已匹配」：跨声明仍必须 fail-closed。
        assert_eq!(unify_call_type_binders(&expected, &ValkyrieType::Utf16, &binders, &mut substitutions, &relations), None);
    }
}
