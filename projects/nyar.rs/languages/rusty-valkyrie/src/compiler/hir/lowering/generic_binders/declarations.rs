use super::propagation::{propagate_declared_generic_binders, propagate_generic_binders_in_block};
use crate::compiler::{
    hir::lowering::{
        diagnostics::{identity_duplicated, index_exhausted, plan_source, plan_span},
        errors::{DiscriminatorFailure, HirError, IndexSpace},
        source_group::SelfOwnerPath,
    },
    types::{
        Identifier, NamePath,
        hir::{
            GenericBinderId, GenericType, HirBlock, HirEnum, HirExpr, HirExprKind, HirField, HirFlagMember, HirFunction, HirImpl, HirModule,
            HirSingleton, HirStatementKind, HirStruct, HirTrait, HirVariant, HirWhereConstraint, ValkyrieType,
        },
    },
};

/// 在结构体/枚举已分配 declaration ItemId 之后，把唯一可解析的 `Named`/`Apply(Named, …)`
/// 绑定为 [`ValkyrieType::Nominal`]。这是 Resolver→HIR 的声明身份冻结，不是短名改写或后置补丁。
pub(crate) fn bind_declared_nominal_types(module: &mut HirModule) -> Result<(), HirError> {
    fn qualified_struct(item: &HirStruct) -> Identifier {
        if item.namespace.is_empty() {
            return item.name.clone();
        }
        let mut parts = item.namespace.iter().map(|part| part.as_str()).collect::<Vec<_>>();
        parts.push(item.name.as_str());
        Identifier::new(&parts.join("."))
    }
    fn collect_structs(module: &HirModule, out: &mut Vec<HirStruct>) {
        out.extend(module.structs.iter().cloned());
        for export in &module.imported_semantic_exports {
            out.extend(export.structs.iter().cloned());
        }
        for submodule in &module.submodules {
            collect_structs(submodule, out);
        }
    }
    fn collect_enums(module: &HirModule, out: &mut Vec<HirEnum>) {
        out.extend(module.enums.iter().cloned());
        for export in &module.imported_semantic_exports {
            out.extend(export.enums.iter().cloned());
        }
        for submodule in &module.submodules {
            collect_enums(submodule, out);
        }
    }
    fn unique_struct<'a>(name: &Identifier, namespace: &NamePath, structs: &'a [HirStruct]) -> Result<Option<&'a HirStruct>, HirError> {
        let unknown_source = nyar_types::DiagnosticSource { id: "unknown".into(), revision: None };
        let unknown_span = nyar_types::DiagnosticSpan { start: 0, end: 0 };
        let exact: Vec<&HirStruct> = structs
            .iter()
            .filter(|item| qualified_struct(item) == *name || (item.name == *name && NamePath::new(item.namespace.clone()) == *namespace))
            .collect();
        if exact.len() > 1 {
            return Err(HirError::NominalAmbiguous {
                name: name.to_string(),
                candidate_count: exact.len(),
                source: unknown_source.clone(),
                span: unknown_span.clone(),
            }
            .into());
        }
        if exact.len() == 1 {
            return Ok(Some(exact[0]));
        }
        let short: Vec<&HirStruct> = structs.iter().filter(|item| &item.name == name).collect();
        if short.len() > 1 {
            return Err(HirError::NominalAmbiguous {
                name: name.to_string(),
                candidate_count: short.len(),
                source: unknown_source.clone(),
                span: unknown_span.clone(),
            }
            .into());
        }
        Ok(short.into_iter().next())
    }
    fn unique_enum<'a>(name: &Identifier, enums: &'a [HirEnum]) -> Result<Option<&'a HirEnum>, HirError> {
        let unknown_source = nyar_types::DiagnosticSource { id: "unknown".into(), revision: None };
        let unknown_span = nyar_types::DiagnosticSpan { start: 0, end: 0 };
        let matches: Vec<&HirEnum> = enums.iter().filter(|item| &item.name == name).collect();
        if matches.len() > 1 {
            return Err(HirError::SumUniteAmbiguous {
                name: name.to_string(),
                candidate_count: matches.len(),
                source: unknown_source.clone(),
                span: unknown_span.clone(),
            }
            .into());
        }
        Ok(matches.into_iter().next())
    }
    fn resolve_declaration(
        name: &Identifier,
        namespace: &NamePath,
        structs: &[HirStruct],
        enums: &[HirEnum],
    ) -> Result<Option<nyar_types::ItemId>, HirError> {
        if let Some(item) = unique_struct(name, namespace, structs)? {
            return Ok(Some(item.constructor_declaration.ok_or_else(|| HirError::StructureMissingConstructor {
                qualified_name: qualified_struct(item).to_string(),
                source: plan_source(),
                span: plan_span(),
            })?));
        }
        if let Some(item) = unique_enum(name, enums)? {
            return Ok(Some(item.declaration.ok_or_else(|| HirError::SumUniteMissingDeclaration {
                name: item.name.to_string(),
                source: plan_source(),
                span: plan_span(),
            })?));
        }
        Ok(None)
    }
    /// `imply` 目标已冻结为声明身份后，`Self` 在该块内指目标声明本身。
    ///
    /// 只对名义目标成立：结构化数组 `[T]` 的 `Self` 不是 `class Array<T>` 名义类型，
    /// 因此失败闭合，不借数组 owner 约定改写。
    fn impl_self_owner(target: &ValkyrieType, structs: &[HirStruct], enums: &[HirEnum]) -> Option<SelfOwnerPath> {
        let declaration = match target {
            ValkyrieType::Nominal { declaration, .. } => *declaration,
            _ => return None,
        };
        if let Some(item) = structs.iter().find(|item| item.constructor_declaration == Some(declaration)) {
            return Some(SelfOwnerPath::from_struct(item));
        }
        enums.iter().find(|item| item.declaration == Some(declaration)).map(|item| SelfOwnerPath::named(&[], &item.name))
    }
    /// 结构化数组 `[T]` 的声明 owner 是否是 `class Array<T>`。
    fn is_array_owner(declaration: nyar_types::ItemId, structs: &[HirStruct]) -> bool {
        array_owner_declarations(structs).contains(&declaration)
    }
    fn bind_type(ty: &mut ValkyrieType, namespace: &NamePath, structs: &[HirStruct], enums: &[HirEnum]) -> Result<(), HirError> {
        match ty {
            ValkyrieType::Nominal { arguments, .. } => {
                for argument in arguments {
                    bind_type(argument, namespace, structs, enums)?;
                }
            }
            ValkyrieType::Named(name) => {
                if let Some(declaration) = resolve_declaration(name, namespace, structs, enums)? {
                    *ty = ValkyrieType::Nominal { declaration, arguments: Vec::new() };
                }
            }
            ValkyrieType::Apply(base, args) => {
                for arg in args.iter_mut() {
                    bind_type(arg, namespace, structs, enums)?;
                }
                let resolved_declaration = match base.as_ref() {
                    ValkyrieType::Named(name) => resolve_declaration(name, namespace, structs, enums)?,
                    _ => None,
                };
                if let Some(declaration) = resolved_declaration {
                    if args.len() == 1 && is_array_owner(declaration, structs) {
                        let element = args.remove(0);
                        *ty = ValkyrieType::Array(Box::new(element));
                        return Ok(());
                    }
                    let arguments = std::mem::take(args);
                    *ty = ValkyrieType::Nominal { declaration, arguments };
                    return Ok(());
                }
                bind_type(base, namespace, structs, enums)?;
            }
            ValkyrieType::Array(inner) | ValkyrieType::Nullable(inner) => bind_type(inner, namespace, structs, enums)?,
            ValkyrieType::FixedArray { element, .. } => bind_type(element, namespace, structs, enums)?,
            ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
                for item in items {
                    bind_type(item, namespace, structs, enums)?;
                }
            }
            ValkyrieType::Function(function) => {
                for param in &mut function.params {
                    bind_type(param, namespace, structs, enums)?;
                }
                bind_type(&mut function.return_type, namespace, structs, enums)?;
            }
            ValkyrieType::Associated(associated) => {
                bind_type(&mut associated.base, namespace, structs, enums)?;
                for argument in &mut associated.type_arguments {
                    bind_type(argument, namespace, structs, enums)?;
                }
            }
            ValkyrieType::TraitObject(object) => {
                for argument in &mut object.type_arguments {
                    bind_type(argument, namespace, structs, enums)?;
                }
            }
            ValkyrieType::Row(row) => {
                for method in &mut row.methods {
                    for param in &mut method.params {
                        bind_type(param, namespace, structs, enums)?;
                    }
                    bind_type(&mut method.return_type, namespace, structs, enums)?;
                }
            }
            ValkyrieType::TypeLambda(lambda) => {
                bind_type(&mut lambda.body, namespace, structs, enums)?;
            }
            _ => {}
        }
        Ok(())
    }
    fn bind_expr(
        expr: &mut HirExpr,
        namespace: &NamePath,
        structs: &[HirStruct],
        enums: &[HirEnum],
        self_owner: Option<&SelfOwnerPath>,
    ) -> Result<(), HirError> {
        match &mut expr.kind {
            HirExprKind::Call { callee, args, resolved, .. } => {
                bind_expr(callee, namespace, structs, enums, self_owner)?;
                for arg in args {
                    bind_expr(&mut arg.value, namespace, structs, enums, self_owner)?;
                }
                if let Some(contract) = resolved {
                    for param in &mut contract.parameter_types {
                        bind_type(param, namespace, structs, enums)?;
                    }
                    bind_type(&mut contract.return_type, namespace, structs, enums)?;
                    if let Some(payload) = &mut contract.extractor_payload_type {
                        bind_type(payload, namespace, structs, enums)?;
                    }
                }
            }
            HirExprKind::Construct { path, args, resolved } => {
                if let Some(owner) = self_owner {
                    if path.parts().len() == 1 && path.parts()[0].as_str() == "Self" {
                        *path = owner.construct_path.clone();
                    }
                }
                for arg in args {
                    bind_expr(arg, namespace, structs, enums, self_owner)?;
                }
                if let Some(contract) = resolved {
                    for param in &mut contract.parameter_types {
                        bind_type(param, namespace, structs, enums)?;
                    }
                    bind_type(&mut contract.return_type, namespace, structs, enums)?;
                }
            }
            HirExprKind::Cast { expr: inner, ty, .. } => {
                bind_expr(inner, namespace, structs, enums, self_owner)?;
                bind_type(ty, namespace, structs, enums)?;
            }
            HirExprKind::Lambda { params, return_type, body, .. } => {
                for param in params {
                    bind_type(&mut param.ty, namespace, structs, enums)?;
                }
                bind_type(return_type, namespace, structs, enums)?;
                bind_block(body, namespace, structs, enums, self_owner)?;
            }
            HirExprKind::TryScope { result_type, body, .. } => {
                if let Some(ty) = result_type {
                    bind_type(ty, namespace, structs, enums)?;
                }
                bind_block(body, namespace, structs, enums, self_owner)?;
            }
            HirExprKind::FieldInit { value, .. }
            | HirExprKind::Await(value)
            | HirExprKind::Awake(value)
            | HirExprKind::BlockOn(value)
            | HirExprKind::YieldFrom(value)
            | HirExprKind::TryPropagate { inner: value, .. }
            | HirExprKind::Raise(value)
            | HirExprKind::Resume(value)
            | HirExprKind::Return(Some(value))
            | HirExprKind::Yield(Some(value))
            | HirExprKind::Break { expr: Some(value), .. }
            | HirExprKind::Assign { value, .. }
            | HirExprKind::FieldAccess { object: value, .. } => bind_expr(value, namespace, structs, enums, self_owner)?,
            HirExprKind::Path(path) => {
                if let Some(owner) = self_owner {
                    if path.parts().len() > 1 && path.parts()[0].as_str() == "Self" {
                        let mut parts = path.parts().to_vec();
                        parts[0] = owner.name.clone();
                        *path = NamePath::new(parts);
                    }
                }
            }
            HirExprKind::GenericApply { callee, arguments } => {
                bind_expr(callee, namespace, structs, enums, self_owner)?;
                for argument in arguments {
                    bind_type(argument, namespace, structs, enums)?;
                }
            }
            HirExprKind::StoreField { object, value, .. } => {
                bind_expr(object, namespace, structs, enums, self_owner)?;
                bind_expr(value, namespace, structs, enums, self_owner)?;
            }
            HirExprKind::ArrayNew { element_type, length } => {
                bind_type(element_type, namespace, structs, enums)?;
                bind_expr(length, namespace, structs, enums, self_owner)?;
            }
            HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } => {
                for item in items {
                    bind_expr(item, namespace, structs, enums, self_owner)?;
                }
            }
            HirExprKind::Block(block) => bind_block(block, namespace, structs, enums, self_owner)?,
            HirExprKind::If { condition, then_branch, else_branch, .. } => {
                bind_expr(condition, namespace, structs, enums, self_owner)?;
                bind_block(then_branch, namespace, structs, enums, self_owner)?;
                if let Some(else_branch) = else_branch {
                    bind_block(else_branch, namespace, structs, enums, self_owner)?;
                }
            }
            HirExprKind::IfLet { pattern: _, scrutinee, then_branch, else_branch, .. } => {
                bind_expr(scrutinee, namespace, structs, enums, self_owner)?;
                bind_block(then_branch, namespace, structs, enums, self_owner)?;
                if let Some(else_branch) = else_branch {
                    bind_block(else_branch, namespace, structs, enums, self_owner)?;
                }
            }
            HirExprKind::Loop { pattern: _, iterator, condition, body, .. } => {
                if let Some(iterator) = iterator {
                    bind_expr(iterator, namespace, structs, enums, self_owner)?;
                }
                if let Some(condition) = condition {
                    bind_expr(condition, namespace, structs, enums, self_owner)?;
                }
                bind_block(body, namespace, structs, enums, self_owner)?;
            }
            HirExprKind::Catch { expr: inner, arms } => {
                bind_expr(inner, namespace, structs, enums, self_owner)?;
                for arm in arms {
                    if let Some(guard) = &mut arm.guard {
                        bind_expr(guard, namespace, structs, enums, self_owner)?;
                    }
                    bind_expr(&mut arm.body, namespace, structs, enums, self_owner)?;
                }
            }
            HirExprKind::With { base, updates } => {
                bind_expr(base, namespace, structs, enums, self_owner)?;
                for (_, update) in updates {
                    bind_expr(update, namespace, structs, enums, self_owner)?;
                }
            }
            HirExprKind::SuperCall { args, .. } => {
                for arg in args {
                    bind_expr(arg, namespace, structs, enums, self_owner)?;
                }
            }
            HirExprKind::Match { scrutinee, arms, .. } | HirExprKind::Case { scrutinee, arms, .. } => {
                bind_expr(scrutinee, namespace, structs, enums, self_owner)?;
                for arm in arms {
                    if let Some(guard) = &mut arm.guard {
                        bind_expr(guard, namespace, structs, enums, self_owner)?;
                    }
                    bind_expr(&mut arm.body, namespace, structs, enums, self_owner)?;
                }
            }
            HirExprKind::AnonymousClass { fields, methods, .. } => {
                for (_, value) in fields {
                    bind_expr(value, namespace, structs, enums, self_owner)?;
                }
                for method in methods {
                    bind_function(method, structs, enums, None)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn bind_block(
        block: &mut HirBlock,
        namespace: &NamePath,
        structs: &[HirStruct],
        enums: &[HirEnum],
        self_owner: Option<&SelfOwnerPath>,
    ) -> Result<(), HirError> {
        for statement in &mut block.statements {
            match &mut statement.kind {
                HirStatementKind::Let { ty, initializer, .. } => {
                    if let Some(annotation) = ty {
                        bind_type(annotation, namespace, structs, enums)?;
                    }
                    if let Some(initializer) = initializer {
                        bind_expr(initializer, namespace, structs, enums, self_owner)?;
                    }
                }
                HirStatementKind::Expr(expr) => bind_expr(expr, namespace, structs, enums, self_owner)?,
            }
        }
        if let Some(expr) = &mut block.expr {
            bind_expr(expr, namespace, structs, enums, self_owner)?;
        }
        Ok(())
    }
    fn bind_function(
        function: &mut HirFunction,
        structs: &[HirStruct],
        enums: &[HirEnum],
        self_owner: Option<&SelfOwnerPath>,
    ) -> Result<(), HirError> {
        for param in &mut function.params {
            bind_type(&mut param.ty, &function.declaring_namespace, structs, enums)?;
        }
        bind_type(&mut function.return_type, &function.declaring_namespace, structs, enums)?;
        for constraint in &mut function.where_constraints {
            bind_type(&mut constraint.target, &function.declaring_namespace, structs, enums)?;
            for bound in &mut constraint.bounds {
                for argument in &mut bound.type_arguments {
                    bind_type(argument, &function.declaring_namespace, structs, enums)?;
                }
                for equation in &mut bound.associated_types {
                    bind_type(&mut equation.ty, &function.declaring_namespace, structs, enums)?;
                }
            }
        }
        bind_block(&mut function.body, &function.declaring_namespace, structs, enums, self_owner)
    }
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    collect_structs(module, &mut structs);
    collect_enums(module, &mut enums);
    for item in &mut module.structs {
        let namespace = NamePath::new(item.namespace.clone());
        for field in &mut item.fields {
            bind_type(&mut field.ty, &namespace, &structs, &enums)?;
            propagate_declared_generic_binders(&mut field.ty, &item.generics);
        }
        let self_owner = SelfOwnerPath::from_struct(item);
        for function in &mut item.methods {
            bind_function(function, &structs, &enums, Some(&self_owner))?;
            propagate_declared_generic_binders(&mut function.return_type, &item.generics);
            for param in &mut function.params {
                propagate_declared_generic_binders(&mut param.ty, &item.generics);
            }
            propagate_generic_binders_in_block(&mut function.body, &item.generics);
            propagate_generic_binders_in_where_constraints(&mut function.where_constraints, &item.generics);
        }
    }
    for enum_def in &mut module.enums {
        for variant in &mut enum_def.variants {
            for field in &mut variant.fields {
                bind_type(&mut field.ty, &NamePath::default(), &structs, &enums)?;
                propagate_declared_generic_binders(&mut field.ty, &enum_def.generics);
            }
            if let Some(result) = &mut variant.result_type {
                bind_type(result, &NamePath::default(), &structs, &enums)?;
                propagate_declared_generic_binders(result, &enum_def.generics);
            }
        }
    }
    for function in &mut module.functions {
        bind_function(function, &structs, &enums, None)?;
        propagate_declared_generic_binders(&mut function.return_type, &function.generics);
        for param in &mut function.params {
            propagate_declared_generic_binders(&mut param.ty, &function.generics);
        }
        propagate_generic_binders_in_block(&mut function.body, &function.generics);
        propagate_generic_binders_in_where_constraints(&mut function.where_constraints, &function.generics);
    }
    for singleton in &mut module.singletons {
        let namespace = NamePath::new(singleton.namespace.clone());
        for field in &mut singleton.fields {
            bind_type(&mut field.ty, &namespace, &structs, &enums)?;
            propagate_declared_generic_binders(&mut field.ty, &singleton.generics);
        }
        let self_owner = SelfOwnerPath::named(&singleton.namespace, &singleton.name);
        for function in &mut singleton.methods {
            bind_function(function, &structs, &enums, Some(&self_owner))?;
            propagate_declared_generic_binders(&mut function.return_type, &singleton.generics);
            for param in &mut function.params {
                propagate_declared_generic_binders(&mut param.ty, &singleton.generics);
            }
            propagate_generic_binders_in_block(&mut function.body, &singleton.generics);
            propagate_generic_binders_in_where_constraints(&mut function.where_constraints, &singleton.generics);
        }
        if let Some(function) = &mut singleton.constructor {
            bind_function(function, &structs, &enums, Some(&self_owner))?;
            propagate_declared_generic_binders(&mut function.return_type, &singleton.generics);
            for param in &mut function.params {
                propagate_declared_generic_binders(&mut param.ty, &singleton.generics);
            }
            propagate_generic_binders_in_block(&mut function.body, &singleton.generics);
            propagate_generic_binders_in_where_constraints(&mut function.where_constraints, &singleton.generics);
        }
        if let Some(function) = &mut singleton.finalizer {
            bind_function(function, &structs, &enums, Some(&self_owner))?;
            propagate_declared_generic_binders(&mut function.return_type, &singleton.generics);
            for param in &mut function.params {
                propagate_declared_generic_binders(&mut param.ty, &singleton.generics);
            }
            propagate_generic_binders_in_block(&mut function.body, &singleton.generics);
            propagate_generic_binders_in_where_constraints(&mut function.where_constraints, &singleton.generics);
        }
    }
    for impl_block in &mut module.impls {
        bind_type(&mut impl_block.target, &NamePath::default(), &structs, &enums)?;
        let self_owner = impl_self_owner(&impl_block.target, &structs, &enums);
        for function in &mut impl_block.methods {
            bind_function(function, &structs, &enums, self_owner.as_ref())?;
        }
    }
    for trait_def in &mut module.traits {
        for function in trait_def.methods.iter_mut().chain(trait_def.default_methods.iter_mut()) {
            bind_function(function, &structs, &enums, None)?;
            propagate_declared_generic_binders(&mut function.return_type, &trait_def.generics);
            for param in &mut function.params {
                propagate_declared_generic_binders(&mut param.ty, &trait_def.generics);
            }
            propagate_generic_binders_in_block(&mut function.body, &trait_def.generics);
            propagate_generic_binders_in_where_constraints(&mut function.where_constraints, &trait_def.generics);
        }
    }
    for submodule in &mut module.submodules {
        bind_declared_nominal_types(submodule)?;
    }
    propagate_impl_owner_type_generics(module);
    Ok(())
}

/// 全部承担结构化数组 `[T]` owner 身份的声明：唯一约定是名为 `Array` 且带一个泛型的聚合。
fn array_owner_declarations(structs: &[HirStruct]) -> Vec<nyar_types::ItemId> {
    structs
        .iter()
        .filter(|item| item.name.as_str() == "Array" && item.generics.len() == 1)
        .filter_map(|item| item.constructor_declaration)
        .collect()
}

/// 模块（含可见依赖导出与子模块）内结构化数组的 owner 声明。
pub(crate) fn array_owner_declaration(module: &HirModule) -> Option<nyar_types::ItemId> {
    let local = array_owner_declarations(&module.structs);
    if let Some(declaration) = local.first() {
        return Some(*declaration);
    }
    for export in &module.imported_semantic_exports {
        if let Some(declaration) = array_owner_declarations(&export.structs).first() {
            return Some(*declaration);
        }
    }
    module.submodules.iter().find_map(array_owner_declaration)
}

pub(crate) fn lookup_owner_generics_by_declaration(module: &HirModule, declaration: nyar_types::ItemId) -> Vec<GenericType> {
    if let Some(item) = module.enums.iter().find(|item| item.declaration == Some(declaration)) {
        return item.generics.clone();
    }
    if let Some(item) = module.structs.iter().find(|item| item.constructor_declaration == Some(declaration)) {
        return item.generics.clone();
    }
    for export in &module.imported_semantic_exports {
        if let Some(item) = export.enums.iter().find(|item| item.declaration == Some(declaration)) {
            return item.generics.clone();
        }
        if let Some(item) = export.structs.iter().find(|item| item.constructor_declaration == Some(declaration)) {
            return item.generics.clone();
        }
    }
    for submodule in &module.submodules {
        let nested = lookup_owner_generics_by_declaration(submodule, declaration);
        if !nested.is_empty() {
            return nested;
        }
    }
    Vec::new()
}

pub(crate) fn combined_impl_generic_context(module: &HirModule, implementation: &HirImpl) -> Vec<GenericType> {
    let mut generics = implementation.generics.clone();
    let owner_generics = match &implementation.target {
        ValkyrieType::Nominal { declaration, .. } => lookup_owner_generics_by_declaration(module, *declaration),
        ValkyrieType::Array(_) => {
            array_owner_declaration(module).map(|declaration| lookup_owner_generics_by_declaration(module, declaration)).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    for owner in owner_generics {
        if !generics.iter().any(|item| item.name == owner.name) {
            generics.push(owner);
        }
    }
    generics
}

pub(crate) fn propagate_generic_binders_in_where_constraints(constraints: &mut [HirWhereConstraint], generics: &[GenericType]) {
    for constraint in constraints {
        propagate_declared_generic_binders(&mut constraint.target, generics);
        for bound in &mut constraint.bounds {
            for argument in &mut bound.type_arguments {
                propagate_declared_generic_binders(argument, generics);
            }
            for equation in &mut bound.associated_types {
                propagate_declared_generic_binders(&mut equation.ty, generics);
            }
        }
    }
}

pub(crate) fn propagate_impl_owner_type_generics(module: &mut HirModule) {
    let bindings = module.impls.iter().map(|implementation| combined_impl_generic_context(module, implementation)).collect::<Vec<_>>();
    for (implementation, generics) in module.impls.iter_mut().zip(bindings) {
        propagate_declared_generic_binders(&mut implementation.target, &generics);
        for method in &mut implementation.methods {
            propagate_declared_generic_binders(&mut method.return_type, &generics);
            for param in &mut method.params {
                propagate_declared_generic_binders(&mut param.ty, &generics);
            }
            // `imply Foo<K>` 的成员 `where K: Trait` 与形参 `K` 是同一个参数：
            // 约束目标必须带上 owner 的 binder，否则调用点的代入与 evidence 查找永远对不上。
            propagate_generic_binders_in_where_constraints(&mut method.where_constraints, &generics);
            propagate_generic_binders_in_block(&mut method.body, &generics);
        }
    }
    for submodule in &mut module.submodules {
        propagate_impl_owner_type_generics(submodule);
    }
}

/// 整份 HIR 模块（含子模块）已冻结 `ItemId` 的最大下标。
pub(crate) fn max_hir_declaration_index(module: &HirModule) -> Option<u32> {
    fn consider(declaration: Option<nyar_types::ItemId>, max: &mut Option<u32>) {
        if let Some(declaration) = declaration {
            let index = declaration.index();
            *max = Some(max.map_or(index, |current| current.max(index)));
        }
    }
    fn walk(module: &HirModule, max: &mut Option<u32>) {
        for function in &module.functions {
            consider(function.declaration, max);
        }
        for structure in &module.structs {
            consider(structure.constructor_declaration, max);
            for method in &structure.methods {
                consider(method.declaration, max);
            }
            for property in &structure.properties {
                for accessor in property.getter.iter().chain(property.setter.iter()) {
                    consider(accessor.declaration, max);
                }
            }
        }
        for enum_definition in &module.enums {
            consider(enum_definition.declaration, max);
            for variant in &enum_definition.variants {
                consider(variant.declaration, max);
            }
        }
        for singleton in &module.singletons {
            for method in
                singleton.methods.iter().chain(singleton.constructor.iter().map(Box::as_ref)).chain(singleton.finalizer.iter().map(Box::as_ref))
            {
                consider(method.declaration, max);
            }
        }
        for trait_definition in &module.traits {
            consider(trait_definition.declaration, max);
            for method in trait_definition.methods.iter().chain(trait_definition.default_methods.iter()) {
                consider(method.declaration, max);
            }
        }
        for implementation in &module.impls {
            for method in &implementation.methods {
                consider(method.declaration, max);
            }
        }
        for widget in &module.widgets {
            for method in &widget.methods {
                consider(method.declaration, max);
            }
        }
        for submodule in &module.submodules {
            walk(submodule, max);
        }
    }
    let mut max = None;
    walk(module, &mut max);
    max
}

/// 整份 HIR 模块（含子模块）已冻结 callable `ItemInstanceId` 的最大下标。
pub(crate) fn max_hir_callable_instance_index(module: &HirModule) -> Option<u32> {
    fn consider(instance: Option<nyar_types::ItemInstanceId>, max: &mut Option<u32>) {
        if let Some(instance) = instance {
            let index = instance.index();
            *max = Some(max.map_or(index, |current| current.max(index)));
        }
    }
    fn walk(module: &HirModule, max: &mut Option<u32>) {
        for function in &module.functions {
            consider(function.instance, max);
        }
        for structure in &module.structs {
            consider(structure.constructor_instance, max);
            for method in &structure.methods {
                consider(method.instance, max);
            }
            for property in &structure.properties {
                for accessor in property.getter.iter().chain(property.setter.iter()) {
                    consider(accessor.instance, max);
                }
            }
        }
        for enum_definition in &module.enums {
            for variant in &enum_definition.variants {
                consider(variant.instance, max);
            }
        }
        for singleton in &module.singletons {
            consider(singleton.accessor_instance, max);
            for method in
                singleton.methods.iter().chain(singleton.constructor.iter().map(Box::as_ref)).chain(singleton.finalizer.iter().map(Box::as_ref))
            {
                consider(method.instance, max);
            }
        }
        for trait_definition in &module.traits {
            for method in trait_definition.methods.iter().chain(trait_definition.default_methods.iter()) {
                consider(method.instance, max);
            }
        }
        for implementation in &module.impls {
            for method in &implementation.methods {
                consider(method.instance, max);
            }
        }
        for widget in &module.widgets {
            for method in &widget.methods {
                consider(method.instance, max);
            }
        }
        for submodule in &module.submodules {
            walk(submodule, max);
        }
    }
    let mut max = None;
    walk(module, &mut max);
    max
}

pub(crate) fn register_function_declarations(module: &mut HirModule, next: &mut u32, next_instance: &mut u32) -> Result<(), HirError> {
    fn register(function: &mut HirFunction, next: &mut u32, next_instance: &mut u32, owner_monomorphic: bool) -> Result<(), HirError> {
        let owner = function.name.to_string();
        if function.declaration.is_some() || function.instance.is_some() {
            return Err(identity_duplicated(IndexSpace::Item, &owner));
        }
        let following = next.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::Item, &owner))?;
        function.declaration = Some(nyar_types::ItemId::from_index(*next).ok_or_else(|| index_exhausted(IndexSpace::Item, &owner))?);
        *next = following;
        if owner_monomorphic {
            let following = next_instance.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::ItemInstance, &owner))?;
            function.instance =
                Some(nyar_types::ItemInstanceId::from_index(*next_instance).ok_or_else(|| index_exhausted(IndexSpace::ItemInstance, &owner))?);
            *next_instance = following;
        }
        Ok(())
    }
    for function in &mut module.functions {
        register(function, next, next_instance, true)?;
    }
    for structure in &mut module.structs {
        let owner = structure.name.to_string();
        if structure.constructor_declaration.is_some() || structure.constructor_instance.is_some() {
            return Err(identity_duplicated(IndexSpace::Item, &owner));
        }
        structure.constructor_declaration =
            Some(nyar_types::ItemId::from_index(*next).ok_or_else(|| index_exhausted(IndexSpace::Item, &owner))?);
        *next = next.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::Item, &owner))?;
        if structure.generics.is_empty() {
            structure.constructor_instance =
                Some(nyar_types::ItemInstanceId::from_index(*next_instance).ok_or_else(|| index_exhausted(IndexSpace::ItemInstance, &owner))?);
            *next_instance = next_instance.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::ItemInstance, &owner))?;
        }
        for method in &mut structure.methods {
            register(method, next, next_instance, structure.generics.is_empty())?;
        }
        for property in &mut structure.properties {
            for accessor in property.getter.iter_mut().chain(property.setter.iter_mut()) {
                register(accessor, next, next_instance, structure.generics.is_empty())?;
            }
        }
    }
    for enum_definition in &mut module.enums {
        let owner = enum_definition.name.to_string();
        if enum_definition.declaration.is_some() {
            return Err(identity_duplicated(IndexSpace::Item, &owner));
        }
        enum_definition.declaration = Some(nyar_types::ItemId::from_index(*next).ok_or_else(|| index_exhausted(IndexSpace::Item, &owner))?);
        *next = next.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::Item, &owner))?;
        for variant in &mut enum_definition.variants {
            let variant_owner = format!("{owner}::{}", variant.name);
            if variant.declaration.is_some() || variant.instance.is_some() {
                return Err(identity_duplicated(IndexSpace::Item, &variant_owner));
            }
            variant.declaration = Some(nyar_types::ItemId::from_index(*next).ok_or_else(|| index_exhausted(IndexSpace::Item, &variant_owner))?);
            *next = next.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::Item, &variant_owner))?;
            if enum_definition.generics.is_empty() {
                variant.instance = Some(
                    nyar_types::ItemInstanceId::from_index(*next_instance)
                        .ok_or_else(|| index_exhausted(IndexSpace::ItemInstance, &variant_owner))?,
                );
                *next_instance = next_instance.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::ItemInstance, &variant_owner))?;
            }
        }
    }
    for singleton in &mut module.singletons {
        let owner = singleton.name.to_string();
        if singleton.accessor_instance.is_some() {
            return Err(identity_duplicated(IndexSpace::ItemInstance, &owner));
        }
        singleton.accessor_instance =
            Some(nyar_types::ItemInstanceId::from_index(*next_instance).ok_or_else(|| index_exhausted(IndexSpace::ItemInstance, &owner))?);
        *next_instance = next_instance.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::ItemInstance, &owner))?;
        for method in singleton
            .methods
            .iter_mut()
            .chain(singleton.constructor.iter_mut().map(Box::as_mut))
            .chain(singleton.finalizer.iter_mut().map(Box::as_mut))
        {
            register(method, next, next_instance, true)?;
        }
    }
    for trait_definition in &mut module.traits {
        let owner = trait_definition.name.to_string();
        if trait_definition.declaration.is_some() {
            return Err(identity_duplicated(IndexSpace::Item, &owner));
        }
        trait_definition.declaration = Some(nyar_types::ItemId::from_index(*next).ok_or_else(|| index_exhausted(IndexSpace::Item, &owner))?);
        *next = next.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::Item, &owner))?;
        for method in trait_definition.methods.iter_mut() {
            register(method, next, next_instance, false)?;
        }
        for method in trait_definition.default_methods.iter_mut() {
            let materialized_default = !method.is_abstract;
            register(method, next, next_instance, materialized_default)?;
        }
    }
    for implementation in &mut module.impls {
        for method in &mut implementation.methods {
            register(method, next, next_instance, implementation.generics.is_empty())?;
        }
    }
    for widget in &mut module.widgets {
        for method in &mut widget.methods {
            register(method, next, next_instance, widget.generics.is_empty())?;
        }
    }
    for submodule in &mut module.submodules {
        register_function_declarations(submodule, next, next_instance)?;
    }
    Ok(())
}

/// 为结构体、枚举变体与 singleton 字段分配全局唯一 [`nyar_types::FieldId`]。
pub(crate) fn register_field_declarations(module: &mut HirModule, next: &mut u32) -> Result<(), HirError> {
    fn register_fields(fields: &mut [HirField], next: &mut u32) -> Result<(), HirError> {
        for field in fields {
            let owner = field.name.to_string();
            if field.id.is_some() {
                return Err(identity_duplicated(IndexSpace::Field, &owner));
            }
            let id = nyar_types::FieldId::from_index(*next).ok_or_else(|| index_exhausted(IndexSpace::Field, &owner))?;
            field.id = Some(id);
            *next = next.checked_add(1).ok_or_else(|| index_exhausted(IndexSpace::Field, &owner))?;
        }
        Ok(())
    }
    for structure in &mut module.structs {
        register_fields(&mut structure.fields, next)?;
    }
    for enum_definition in &mut module.enums {
        for variant in &mut enum_definition.variants {
            register_fields(&mut variant.fields, next)?;
        }
    }
    for singleton in &mut module.singletons {
        register_fields(&mut singleton.fields, next)?;
    }
    for submodule in &mut module.submodules {
        register_field_declarations(submodule, next)?;
    }
    Ok(())
}
