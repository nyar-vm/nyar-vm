//! 类型辅助函数与表达式类型推断。

use std::collections::BTreeMap;

use crate::compiler::{
    hir::type_relation::TypeRelationContext,
    mir::AggregateFieldIndex,
    types::{
        Identifier,
        hir::{
            GenericType, HirExpr, HirExprKind, HirIdentifier, HirLiteral, HirStatement, HirStatementKind, HirStringSegment,
            PatternRefutability, ValkyrieType,
        },
    },
};

/// 枚举类型的 nominal 形状（泛型参数化为 Generic binder）。
pub(crate) fn enum_apply_type(enum_def: &crate::compiler::types::hir::HirEnum) -> ValkyrieType {
    let arguments = enum_def.generics.iter().map(|generic| ValkyrieType::Generic(generic.clone())).collect::<Vec<_>>();
    if let Some(declaration) = enum_def.declaration {
        return ValkyrieType::Nominal { declaration, arguments };
    }
    // 缺 declaration 时不得伪装成功身份；保留表面 Named 供诊断，后续绑定/校验应失败。
    if arguments.is_empty() {
        ValkyrieType::Named(enum_def.name.clone())
    }
    else {
        ValkyrieType::Apply(Box::new(ValkyrieType::Named(enum_def.name.clone())), arguments)
    }
}

pub(crate) fn struct_apply_type(struct_def: &crate::compiler::types::hir::HirStruct) -> ValkyrieType {
    let arguments = struct_def.generics.iter().map(|generic| ValkyrieType::Generic(generic.clone())).collect::<Vec<_>>();
    if let Some(declaration) = struct_def.constructor_declaration {
        return ValkyrieType::Nominal { declaration, arguments };
    }
    if arguments.is_empty() {
        ValkyrieType::Named(struct_def.name.clone())
    }
    else {
        ValkyrieType::Apply(Box::new(ValkyrieType::Named(struct_def.name.clone())), arguments)
    }
}

pub(crate) fn variant_payload_type(variant: &crate::compiler::types::hir::HirVariant) -> ValkyrieType {
    if !variant.fields.is_empty() {
        if variant.fields.len() == 1 {
            return variant.fields[0].ty.clone();
        }
        return ValkyrieType::Tuple(variant.fields.iter().map(|field| field.ty.clone()).collect());
    }
    ValkyrieType::Unit
}

pub(crate) fn nullable_union_type(payload: ValkyrieType) -> ValkyrieType {
    ValkyrieType::Nullable(Box::new(payload))
}

/// String literals infer as `Utf8` but core APIs take `text`.
pub(crate) fn is_text_type(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Utf8 | ValkyrieType::Utf16 => true,
        ValkyrieType::Named(name) => {
            matches!(name.as_str(), "text" | "utf8" | "Utf8Text" | "utf16" | "Utf16Text" | "core::text::text" | "core.text.text")
        }
        _ => false,
    }
}

/// 空数组字面量：自身没有元素类型事实，只能由调用形式的参数类型定型。
pub(crate) fn is_empty_array_literal(expr: &HirExpr) -> bool {
    matches!(&expr.kind, HirExprKind::ArrayLiteral { items } if items.is_empty())
}

/// 形式参数是否是空数组字面量可以落入的数组形状。
pub(crate) fn accepts_array_shape(expected: &ValkyrieType) -> bool {
    match expected {
        ValkyrieType::Array(_) => true,
        ValkyrieType::FixedArray { length, .. } => *length == 0,
        _ => false,
    }
}

/// 字段身份的唯一冻结入口：按已注册聚合布局读取 `HirField::id`，不在此重新编号。
pub(crate) fn field_identity_from_type(
    object_type: &ValkyrieType,
    field: &Identifier,
    struct_fields: &AggregateFieldIndex,
) -> Option<nyar_types::FieldId> {
    struct_fields.fields_of(object_type)?.iter().find(|item| item.name == *field)?.id
}

pub(crate) fn lookup_struct_field_type(
    object_type: &ValkyrieType,
    field: &Identifier,
    struct_fields: &AggregateFieldIndex,
) -> Option<ValkyrieType> {
    let declared = struct_fields.fields_of(object_type)?.iter().find(|item| item.name == *field)?.ty.clone();
    let generics = struct_fields.generics_of(object_type).unwrap_or(&[]);
    Some(instantiate_declared_type(&declared, generics, object_type.nominal_arguments()))
}

/// 把对象类型的实参代入声明泛型形参，得到字段在该实例上的类型。
///
/// 结构体字段按声明形参书写（`Entry<K, V>` 的 `key: K`）；投影到 `Entry<X, Y>`
/// 上时字段类型必须是 `X`，否则调用点约束里的 `K` 与字段类型里的 `K` 是两个 binder。
fn instantiate_declared_type(declared: &ValkyrieType, generics: &[GenericType], arguments: &[ValkyrieType]) -> ValkyrieType {
    let substitutions = generics.iter().filter_map(|generic| generic.binder).zip(arguments.iter().cloned()).collect::<BTreeMap<_, _>>();
    if substitutions.is_empty() {
        return declared.clone();
    }
    crate::compiler::hir::overload::substitute_type_vars(declared, &substitutions)
}

pub(crate) fn payload_field_scrutinee_types(payload: &ValkyrieType, struct_fields: &AggregateFieldIndex) -> Vec<Option<ValkyrieType>> {
    match payload {
        ValkyrieType::Tuple(items) => items.iter().map(|item| Some(item.clone())).collect(),
        ValkyrieType::Array(inner) => vec![Some(inner.as_ref().clone())],
        ValkyrieType::Named(_) | ValkyrieType::Nominal { .. } | ValkyrieType::Apply(_, _) => match struct_fields.fields_of(payload) {
            Some(fields) if !fields.is_empty() => fields.iter().map(|field| Some(field.ty.clone())).collect(),
            _ => vec![Some(payload.clone())],
        },
        other => vec![Some(other.clone())],
    }
}

/// 查找 object pattern 字段的 scrutinee 类型。
pub(crate) fn object_field_scrutinee_type<'a>(
    object_name: Option<&crate::compiler::types::NamePath>,
    field_name: &Identifier,
    struct_fields: &'a AggregateFieldIndex,
) -> Option<&'a ValkyrieType> {
    let type_name = object_name.and_then(|path| path.parts().last())?;
    struct_fields.fields_of(&ValkyrieType::Named(type_name.clone()))?.iter().find(|field| &field.name == field_name).map(|field| &field.ty)
}

/// 把局部根的 `a.b.c` 点分路径规范化为嵌套 `FieldAccess`，并冻结每级字段身份。
pub(crate) fn normalize_local_field_chain(expr: &mut HirExpr, locals: &BTreeMap<String, ValkyrieType>, struct_fields: &AggregateFieldIndex) {
    let HirExprKind::Path(path) = &expr.kind
    else {
        return;
    };
    let parts = path.parts().to_vec();
    if parts.len() < 2 {
        return;
    }
    let Some(mut current_type) = locals.get(parts[0].as_str()).cloned()
    else {
        return;
    };
    let span = expr.span.clone();
    let mut current = HirExpr {
        kind: HirExprKind::Variable(HirIdentifier { name: parts[0].clone(), shadow_index: 0, span: span.clone() }),
        span: span.clone(),
    };
    for part in &parts[1..] {
        let field_id = field_identity_from_type(&current_type, part, struct_fields);
        current_type = lookup_struct_field_type(&current_type, part, struct_fields).unwrap_or(ValkyrieType::AutoType);
        current = HirExpr { kind: HirExprKind::FieldAccess { object: Box::new(current), field: part.clone(), field_id }, span: span.clone() };
    }
    expr.kind = current.kind;
}

/// 单段小写名字模式只能是 Variable；如果是 Name 形式则应解析为类型测试模式。
pub(crate) fn should_resolve_name_pattern_as_type(name: &crate::compiler::types::NamePath, actual_type: &ValkyrieType) -> bool {
    if name.parts().len() != 1 {
        return false;
    }
    match actual_type {
        ValkyrieType::Named(actual_name) => name.parts().last().is_some_and(|expected| expected == actual_name),
        ValkyrieType::Apply(base, _) => should_resolve_name_pattern_as_type(name, base),
        _ => false,
    }
}

pub(crate) fn pattern_type_declaration(
    path: &crate::compiler::types::NamePath,
    type_relations: &TypeRelationContext,
) -> Option<nyar_types::ItemId> {
    let [name] = path.parts()
    else {
        return None;
    };
    type_relations.nominal_type_for_name(name).and_then(|ty| ty.nominal_declaration())
}

pub(crate) fn canonical_extractor_callee(name: &crate::compiler::types::NamePath) -> crate::compiler::types::NamePath {
    let mut parts = name.parts().to_vec();
    parts.push(Identifier::new("extractor"));
    crate::compiler::types::NamePath::new(parts)
}

pub(crate) fn is_null_type(ty: &ValkyrieType) -> bool {
    matches!(ty, ValkyrieType::Named(name) if name.as_str() == "null")
}

pub(crate) fn infer_catch_scrutinee_type(expr: &HirExpr, locals: &BTreeMap<String, ValkyrieType>) -> Option<ValkyrieType> {
    match &expr.kind {
        HirExprKind::Raise(inner) => infer_expr_type(inner, locals),
        HirExprKind::Block(block) => block.expr.as_ref().and_then(|inner| infer_catch_scrutinee_type(inner, locals)),
        _ => infer_expr_type(expr, locals),
    }
}

/// 从被调表达式提取可调用名称；外部测试入口。
#[doc(hidden)]
pub fn extract_callable_name(callee: &HirExpr) -> Option<Identifier> {
    match &callee.kind {
        HirExprKind::Variable(identifier) => Some(identifier.name.clone()),
        HirExprKind::Path(path) => path.parts().last().cloned(),
        HirExprKind::FieldAccess { field, .. } => Some(field.clone()),
        HirExprKind::GenericApply { callee, .. } => extract_callable_name(callee),
        _ => None,
    }
}

/// 依据已解析调用合同推断表达式类型；外部测试入口。
#[doc(hidden)]
pub fn infer_expr_type(expr: &HirExpr, locals: &BTreeMap<String, ValkyrieType>) -> Option<ValkyrieType> {
    match &expr.kind {
        HirExprKind::Literal(literal) => Some(match literal {
            // Unsuffixed integer literals default to signed i32 when they fit;
            // matches MIR `lower_literal` and `return -1` (`prefix -` of `1`).
            crate::compiler::types::hir::HirLiteral::Integer64(value) if *value >= i32::MIN as i64 && *value <= i32::MAX as i64 => {
                ValkyrieType::Integer32 { signed: true }
            }
            crate::compiler::types::hir::HirLiteral::Integer64(_) => ValkyrieType::Integer64 { signed: true },
            crate::compiler::types::hir::HirLiteral::Float64(_) => ValkyrieType::Float64,
            crate::compiler::types::hir::HirLiteral::String(_) => ValkyrieType::Utf8,
            crate::compiler::types::hir::HirLiteral::Bool(_) => ValkyrieType::Boolean,
            crate::compiler::types::hir::HirLiteral::Unit => ValkyrieType::Unit,
            // 空值没有独立名义类型；其 `T?` 来自使用位置的上下文。
            crate::compiler::types::hir::HirLiteral::Null => return None,
        }),
        HirExprKind::Variable(HirIdentifier { name, .. }) => locals.get(name.as_str()).cloned(),
        // The parser may retain a bare local reference as a single-segment
        // Path instead of normalizing it to Variable. Both forms denote the
        // same lexical binding, so semantic resolution must consult the same
        // typed local environment before considering any call contract.
        HirExprKind::Path(path) if path.parts().len() == 1 => locals.get(path.parts()[0].as_str()).cloned(),
        HirExprKind::Call { resolved, .. } => resolved.as_ref().map(|call| call.return_type.clone()),
        HirExprKind::ArrayLiteral { items } => {
            let item_type = items.first().and_then(|item| infer_expr_type(item, locals))?;
            Some(ValkyrieType::Array(Box::new(item_type)))
        }
        HirExprKind::TupleLiteral { items } => {
            let element_types = items.iter().map(|item| infer_expr_type(item, locals)).collect::<Option<Vec<_>>>()?;
            Some(ValkyrieType::Tuple(element_types))
        }
        // 集合字面量构造结果的静态类型即容器类型。
        HirExprKind::CollectionInit { container_type, .. } => Some(container_type.clone()),
        HirExprKind::Construct { resolved, .. } => resolved.as_ref().map(|call| call.return_type.clone()),
        HirExprKind::If { then_branch, else_branch, .. } | HirExprKind::IfLet { then_branch, else_branch, .. } => {
            let then_type = then_branch.expr.as_ref().and_then(|expr| infer_expr_type(expr, locals))?;
            let else_type = else_branch.as_ref().and_then(|branch| branch.expr.as_ref()).and_then(|expr| infer_expr_type(expr, locals))?;
            if then_type == else_type { Some(then_type) } else { None }
        }
        HirExprKind::Block(block) => block.expr.as_ref().and_then(|expr| infer_expr_type(expr, locals)),
        HirExprKind::Cast { ty, .. } => Some(ty.clone()),
        HirExprKind::TryPropagate { inner, .. } => {
            infer_expr_type(inner, locals).and_then(|ty| crate::compiler::hir::try_propagate::nullable_payload_type(&ty))
        }
        HirExprKind::TryScope { is_optional, is_forced, result_type, body } => {
            if let Some(ty) = result_type {
                Some(ty.clone())
            }
            else {
                let body_type = body.expr.as_ref().and_then(|expr| infer_expr_type(expr, locals))?;
                if *is_optional {
                    Some(ValkyrieType::Nullable(Box::new(body_type)))
                }
                else if *is_forced {
                    Some(body_type)
                }
                else {
                    Some(body_type)
                }
            }
        }
        _ => None,
    }
}

/// 推断 `match` 被检表达式的类型；外部测试入口。
#[doc(hidden)]
pub fn infer_scrutinee_type(
    expr: &HirExpr,
    locals: &BTreeMap<String, ValkyrieType>,
    struct_fields: &AggregateFieldIndex,
    singleton_names: &std::collections::BTreeSet<Identifier>,
) -> Option<ValkyrieType> {
    if let Some(ty) = infer_expr_type(expr, locals) {
        return Some(ty);
    }
    match &expr.kind {
        HirExprKind::Variable(identifier) if singleton_names.contains(&identifier.name) => Some(ValkyrieType::Named(identifier.name.clone())),
        HirExprKind::Path(path) if path.parts().len() == 1 && singleton_names.contains(&path.parts()[0]) => {
            Some(ValkyrieType::Named(path.parts()[0].clone()))
        }
        // Parser-preserved `binding.field[.field...]` paths are equivalent to
        // nested FieldAccess nodes. Project each field through the declared
        // aggregate layout; a missing segment stays unresolved rather than
        // borrowing a result type from the surrounding expression.
        HirExprKind::Path(path) if path.parts().len() >= 2 => {
            let mut current = locals.get(path.parts()[0].as_str())?.clone();
            for field in &path.parts()[1..] {
                current = lookup_struct_field_type(&current, field, struct_fields)?;
            }
            Some(current)
        }
        HirExprKind::FieldAccess { object, field, .. } => {
            if let Some(singleton_name) =
                crate::compiler::hir::overload::substitution::extract_singleton_type_name(object, locals, singleton_names)
            {
                return lookup_struct_field_type(&ValkyrieType::Named(singleton_name), field, struct_fields);
            }
            let object_type = infer_scrutinee_type(object, locals, struct_fields, singleton_names)?;
            lookup_struct_field_type(&object_type, field, struct_fields)
        }
        _ => None,
    }
}

pub(crate) fn compute_call_match_kind(
    type_relations: &TypeRelationContext,
    actual_types: &[ValkyrieType],
    expected_types: &[ValkyrieType],
) -> Option<crate::compiler::hir::overload::diagnostics::OverloadMatchKind> {
    use crate::compiler::hir::{overload::diagnostics::OverloadMatchKind, type_relation::ParameterMatchResult};

    if actual_types.len() != expected_types.len() {
        return None;
    }

    let mut saw_trait = false;
    let mut saw_row = false;
    let mut subtype_distance = 0usize;

    for (actual, expected) in actual_types.iter().zip(expected_types) {
        match type_relations.match_parameter(actual, expected) {
            ParameterMatchResult::NominalExact => {}
            ParameterMatchResult::NominalSubtype { distance } => subtype_distance += distance,
            ParameterMatchResult::Trait { .. } => saw_trait = true,
            ParameterMatchResult::Row => saw_row = true,
            ParameterMatchResult::NoMatch { .. } => {
                return None;
            }
        }
    }

    if saw_row {
        Some(OverloadMatchKind::Row)
    }
    else if saw_trait {
        Some(OverloadMatchKind::Trait)
    }
    else if subtype_distance > 0 {
        Some(OverloadMatchKind::NominalSubtype { distance: subtype_distance })
    }
    else {
        Some(OverloadMatchKind::NominalExact)
    }
}

pub(crate) fn render_valkyrie_type_name(ty: &ValkyrieType) -> String {
    match ty {
        ValkyrieType::Named(name) => name.as_str().to_string(),
        ValkyrieType::Tuple(items) => {
            let inner = items.iter().map(render_valkyrie_type_name).collect::<Vec<_>>().join(", ");
            format!("({inner})")
        }
        ValkyrieType::Array(inner) => format!("Array<{}>", render_valkyrie_type_name(inner)),
        ValkyrieType::Union(items) => items.iter().map(render_valkyrie_type_name).collect::<Vec<_>>().join(" | "),
        ValkyrieType::Apply(base, args) => {
            let rendered_args = args.iter().map(render_valkyrie_type_name).collect::<Vec<_>>().join(", ");
            format!("{}<{}>", render_valkyrie_type_name(base), rendered_args)
        }
        ValkyrieType::r#SelfType => "Self".to_string(),
        other => format!("{other:?}"),
    }
}
