use std::ops::Range;

use crate::compiler::{
    hir::{lower_type_expression, render_type_expression},
    mir::{SINGLETON_CONSTRUCTOR_NAME, SINGLETON_FINALIZER_NAME},
    semantic::operator_members,
    syntax::oak::{
        self as frontend,
        ast::{
            AssociatedType, Attribute, ClassDeclaration, EnumVariant, Enums, FieldDeclaration, Flags, GenericParam, ImplyDeclaration,
            MethodDeclaration, NamePath as AstNamePath, Param, Parent, SingletonDeclaration, StatementNode, StructureDeclaration,
            TermExpression, Trait, TypeExpression, TypeFunction, UsingDeclaration, WidgetDeclaration,
        },
    },
    types::{
        Identifier, NamePath, SourceID, SourceSpan,
        hir::{
            GenericType, HirArgument, HirAssociatedType, HirAttribute, HirBlock, HirDocumentation, HirEnum, HirExpr, HirExprKind, HirField,
            HirFlagMember, HirFunction, HirImport, HirImportBinding, HirLiteral, HirParam, HirParameterBindingKind, HirParent, HirProperty,
            HirSingleton, HirStatement, HirStatementKind, HirTraitBound, HirTypeAlias, HirTypeFunction, HirVariadicKind, HirVariant,
            HirVisibility, HirWhereConstraint,
        },
    },
};

use crate::compiler::hir::lowering::{diagnostics::plan_source, errors::HirError, expr_lowering::extract_name_path};

use crate::compiler::types::hir::{HirKind, ValkyrieType};

/// Extract `[tag(N)]` / `[tag(N, default)]` into a discriminator literal for `unite` layouts.
pub(crate) fn tag_attribute_discriminator(annotations: &[Attribute], source_id: SourceID, span: Range<usize>) -> Option<HirExpr> {
    for attribute in annotations {
        if attribute.name.name != "tag" {
            continue;
        }
        let first = attribute.args.first()?;
        return crate::compiler::hir::lowering::expr_lowering::lower_term_expression(&first.value, source_id, span).ok();
    }
    None
}

pub(crate) fn lower_trait_associated_type(item: &AssociatedType, source_id: SourceID) -> Result<HirAssociatedType, HirError> {
    Ok(HirAssociatedType {
        name: Identifier::new(&item.name.name),
        doc: lower_documentation(&item.annotations),
        type_params: Vec::new(),
        bounds: item.bounds.iter().map(lower_type_expression).collect::<Result<Vec<_>, _>>()?,
        default: item.default.as_ref().map(lower_type_expression).transpose()?,
        span: with_source(&frontend::std_range(&item.span), source_id),
    })
}

pub(crate) fn lower_attribute(attribute: &Attribute, source_id: SourceID, fallback_span: Range<usize>) -> Result<HirAttribute, HirError> {
    let arguments = attribute
        .args
        .iter()
        .map(|argument| -> Result<_, HirError> {
            Ok(HirArgument {
                key: argument.key.as_ref().map(|key| Identifier::new(&key.name)),
                value: Box::new(lower_attribute_argument_expression(&argument.value, source_id, fallback_span.clone())?),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HirAttribute::with_arguments(NamePath::new(vec![oak_identifier(&attribute.name)]), arguments))
}

pub(crate) fn lower_attribute_argument_expression(
    expr: &TermExpression,
    source_id: SourceID,
    fallback_span: Range<usize>,
) -> Result<HirExpr, HirError> {
    match expr {
        TermExpression::NamePath(path) => {
            Ok(HirExpr { kind: HirExprKind::Path(lower_name_path(path)), span: with_source(&frontend::std_range(&path.span), source_id) })
        }
        _ => crate::compiler::hir::lowering::expr_lowering::lower_term_expression(expr, source_id, fallback_span),
    }
}

pub(crate) fn lower_documentation(_annotations: &[Attribute]) -> HirDocumentation {
    HirDocumentation::default()
}

pub(crate) fn lower_visibility(annotations: &[Attribute]) -> HirVisibility {
    if has_modifier(annotations, "private") {
        HirVisibility::private()
    }
    else if has_modifier(annotations, "protected") {
        HirVisibility::protected()
    }
    else if has_modifier(annotations, "internal") {
        HirVisibility::internal()
    }
    else {
        HirVisibility::public()
    }
}

pub(crate) fn has_modifier(annotations: &[Attribute], name: &str) -> bool {
    annotations.iter().any(|attribute| attribute.name.name == name)
}

pub(crate) fn lower_derives(annotations: &[Attribute]) -> Vec<NamePath> {
    annotations
        .iter()
        .find(|attribute| attribute.name.name == "derive")
        .map(|attribute| attribute.args.iter().filter_map(|argument| extract_name_path(&argument.value)).collect())
        .unwrap_or_default()
}

pub(crate) fn lower_parent(parent: &Parent) -> HirParent {
    HirParent::full(lower_name_path(&parent.name), parent.alias.as_ref().map(|alias| Identifier::new(&alias.name)), Vec::new())
}

pub(crate) fn lower_field(field: &FieldDeclaration) -> Result<HirField, HirError> {
    Ok(HirField {
        id: None,
        name: Identifier::new(&field.name.name),
        doc: lower_documentation(&field.annotations),
        ty: lower_type_expression(&field.typing)?,
        visibility: lower_visibility(&field.annotations),
        is_mutable: has_modifier(&field.annotations, "mut"),
    })
}

pub(crate) fn lower_generic_parameters(parameters: &[GenericParam]) -> Vec<GenericType> {
    parameters.iter().map(lower_generic_parameter).collect()
}

/// 声明域拥有的泛型形参短名；用于 [`GenericBinderScope`] 的可见性事实。
pub(crate) fn generic_parameter_names(parameters: &[GenericParam]) -> Vec<String> {
    parameters.iter().map(|parameter| parameter.name.name.clone()).collect()
}

/// `imply Option<T>` 的 `<T>` 是 impl 的类型形参声明位置。
pub(crate) fn imply_generic_parameter_names(imply: &ImplyDeclaration) -> Vec<String> {
    fn collect(ty: &TypeExpression, out: &mut Vec<String>) {
        match ty {
            TypeExpression::Namepath(path) if path.parts.len() == 1 => {
                let name = path.parts[0].name.as_str();
                if !crate::compiler::hir::is_bound_type_name(name) && !out.iter().any(|existing| existing == name) {
                    out.push(name.to_string());
                }
            }
            TypeExpression::Apply(apply) => {
                for argument in &apply.arguments {
                    collect(argument, out);
                }
            }
            _ => {}
        }
    }
    let mut names = generic_parameter_names(&imply.generics);
    if let TypeExpression::Apply(apply) = &imply.target_type {
        for argument in &apply.arguments {
            collect(argument, &mut names);
        }
    }
    names
}

pub(crate) fn lower_generic_parameter(parameter: &GenericParam) -> GenericType {
    GenericType {
        binder: None,
        name: Identifier::new(&parameter.name.name),
        kind: HirKind::Type,
        bounds: parameter.constraints.iter().map(lower_bound_identifier).collect(),
    }
}

pub(crate) fn lower_bound_identifier(bound: &frontend::ast::TypeExpression) -> Identifier {
    Identifier::new(&render_type_expression(bound))
}

pub(crate) fn lower_param(param: &Param, source_id: SourceID, fallback_span: Range<usize>) -> Result<HirParam, HirError> {
    let span_range = if param.span.is_empty() { fallback_span } else { frontend::std_range(&param.span) };
    Ok(HirParam {
        name: crate::compiler::types::hir::HirIdentifier {
            name: Identifier::new(&param.name.name),
            shadow_index: 0,
            span: with_source(&span_range, source_id),
        },
        ty: param.ty.as_ref().map(lower_type_expression).transpose()?.unwrap_or(ValkyrieType::AutoType),
        binding_kind: HirParameterBindingKind::PositionalOrKeyword,
        is_mutable: has_modifier(&param.annotations, "mut"),
        default: param
            .default
            .as_ref()
            .map(|expr| crate::compiler::hir::lowering::expr_lowering::lower_term_expression(expr, source_id, span_range.clone()))
            .transpose()?,
        variadic: HirVariadicKind::None,
    })
}

pub(crate) fn lower_method_params(method: &MethodDeclaration, source_id: SourceID) -> Result<Vec<HirParam>, HirError> {
    let span = frontend::std_range(&method.span);
    method.parameters.iter().map(|param| lower_param(param, source_id, span.clone())).collect()
}

pub(crate) fn lower_property_params(method: &MethodDeclaration, source_id: SourceID) -> Result<Vec<HirParam>, HirError> {
    lower_method_params(method, source_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropertyMethodKind {
    Get,
    Set,
}

pub(crate) fn property_accessor_kind(method: &MethodDeclaration) -> Option<PropertyMethodKind> {
    if has_modifier(&method.annotations, "get") {
        Some(PropertyMethodKind::Get)
    }
    else if has_modifier(&method.annotations, "set") {
        Some(PropertyMethodKind::Set)
    }
    else {
        None
    }
}

pub(crate) fn is_property_accessor(method: &MethodDeclaration) -> bool {
    property_accessor_kind(method).is_some()
}

pub(crate) fn lower_property_type(method: &MethodDeclaration, accessor_kind: PropertyMethodKind) -> Result<ValkyrieType, HirError> {
    Ok(match accessor_kind {
        PropertyMethodKind::Get => method.return_type.as_ref().map(lower_type_expression).transpose()?.unwrap_or(ValkyrieType::Unit),
        PropertyMethodKind::Set => {
            method.parameters.last().and_then(|param| param.ty.as_ref().map(lower_type_expression)).transpose()?.unwrap_or(ValkyrieType::Unit)
        }
    })
}

pub(crate) fn property_is_abstract(method: &MethodDeclaration) -> bool {
    method.body.is_none() || has_modifier(&method.annotations, "abstract")
}

pub(crate) fn property_is_final(method: &MethodDeclaration) -> bool {
    has_modifier(&method.annotations, "final")
}

pub(crate) fn property_is_static(method: &MethodDeclaration) -> bool {
    has_modifier(&method.annotations, "static")
}

pub(crate) fn property_is_virtual(method: &MethodDeclaration) -> bool {
    has_modifier(&method.annotations, "virtual")
}

pub(crate) fn property_is_override(method: &MethodDeclaration) -> bool {
    has_modifier(&method.annotations, "override")
}

pub(crate) fn property_is_lazy(method: &MethodDeclaration) -> bool {
    has_modifier(&method.annotations, "lazy")
}

pub(crate) fn lower_using(using: &UsingDeclaration) -> HirImport {
    HirImport {
        path: lower_name_path(&using.path),
        alias: using.alias.as_ref().map(|alias| Identifier::new(&alias.name)),
        bindings: using.imports.iter().map(|item| HirImportBinding { name: Identifier::new(&item.name), alias: None }).collect(),
        glob: false,
    }
}

pub(crate) fn lower_trait_path_from_type(ty: &TypeExpression) -> NamePath {
    match ty {
        TypeExpression::Namepath(path) => lower_name_path(path),
        TypeExpression::Apply(node) => lower_trait_path_from_type(&node.base),
        other => NamePath::new(vec![Identifier::new(&render_type_expression(other))]),
    }
}

pub(crate) fn lower_name_path(path: &AstNamePath) -> NamePath {
    let parts = path.parts.iter().flat_map(|part| part.name.split('.').filter(|segment| !segment.is_empty())).map(Identifier::new).collect();
    NamePath::new(parts)
}

pub(crate) fn default_module_name() -> NamePath {
    NamePath::new(vec![Identifier::new("main")])
}

pub(crate) fn with_source(span: &Range<usize>, source_id: SourceID) -> SourceSpan {
    SourceSpan::new(source_id, span.start as u32, span.end as u32)
}

pub(crate) fn oak_identifier(id: &frontend::ast::Identifier) -> Identifier {
    Identifier::new(&id.name)
}

/// 运算符声明（`infix \`+\`` / `prefix \`-\`` / `suffix \`[]\``）的 HIR 成员名。
pub(crate) fn operator_member_name(declared: &frontend::ast::Identifier, annotations: &[Attribute]) -> Identifier {
    Identifier::new(&operator_members::declared_member_name(&declared.name, annotations))
}

pub(crate) fn lower_where_constraints(
    constraints: &[oak_valkyrie::ast::type_nodes::WhereConstraint],
    source_id: SourceID,
) -> Result<Vec<HirWhereConstraint>, HirError> {
    constraints
        .iter()
        .map(|constraint| {
            let target = lower_type_expression(&constraint.target)?;
            let bounds = constraint
                .bounds
                .iter()
                .map(|bound| {
                    let type_arguments = bound.type_arguments.iter().map(lower_type_expression).collect::<Result<Vec<_>, _>>()?;
                    let associated_types = bound
                        .associated_types
                        .iter()
                        .map(|equation| -> Result<_, HirError> {
                            Ok(crate::compiler::types::hir::HirAssociatedTypeBinding {
                                name: oak_identifier(&equation.name),
                                ty: lower_type_expression(&equation.ty)?,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok::<_, HirError>(HirTraitBound {
                        declaration: None,
                        trait_path: NamePath::new(bound.trait_path.parts.iter().map(oak_identifier).collect()),
                        type_arguments,
                        associated_types,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(HirWhereConstraint { target, bounds, span: with_source(&frontend::std_range(&constraint.span), source_id) })
        })
        .collect()
}
