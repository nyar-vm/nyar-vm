use super::propagate_generic_binders_in_where_constraints;
use crate::compiler::{
    hir::lowering::{
        diagnostics::{plan_source, plan_span},
        errors::HirError,
    },
    types::{
        Identifier,
        hir::{
            GenericBinderId, GenericType, HirBlock, HirEnum, HirExpr, HirExprKind, HirFunction, HirImpl, HirModule, HirStatementKind, HirStruct,
            HirTrait, ValkyrieType,
        },
    },
};

pub(crate) fn freeze_generic_binders(generics: &mut [GenericType], owner: nyar_types::ItemId) {
    for (ordinal, generic) in generics.iter_mut().enumerate() {
        if generic.binder.is_none() {
            generic.binder = Some(GenericBinderId { owner, ordinal: ordinal as u32 });
        }
    }
}

pub(crate) fn propagate_declared_generic_binders(ty: &mut ValkyrieType, generics: &[GenericType]) {
    match ty {
        ValkyrieType::Named(name) => {
            if let Some(declared) = generics.iter().find(|item| item.name == *name) {
                *ty = ValkyrieType::Generic(declared.clone());
            }
        }
        ValkyrieType::Generic(generic) => {
            if generic.binder.is_none() {
                if let Some(declared) = generics.iter().find(|item| item.name == generic.name) {
                    generic.binder = declared.binder;
                    generic.bounds = declared.bounds.clone();
                }
            }
        }
        ValkyrieType::Nominal { arguments, .. } | ValkyrieType::Apply(_, arguments) | ValkyrieType::Tuple(arguments) => {
            for argument in arguments {
                propagate_declared_generic_binders(argument, generics);
            }
        }
        ValkyrieType::Array(inner) | ValkyrieType::Nullable(inner) | ValkyrieType::FixedArray { element: inner, .. } => {
            propagate_declared_generic_binders(inner, generics);
        }
        ValkyrieType::Union(arguments) | ValkyrieType::Intersection(arguments) => {
            for argument in arguments {
                propagate_declared_generic_binders(argument, generics);
            }
        }
        ValkyrieType::Function(function) => {
            for param in &mut function.params {
                propagate_declared_generic_binders(param, generics);
            }
            propagate_declared_generic_binders(&mut function.return_type, generics);
        }
        ValkyrieType::Associated(associated) => {
            propagate_declared_generic_binders(&mut associated.base, generics);
            for argument in &mut associated.type_arguments {
                propagate_declared_generic_binders(argument, generics);
            }
        }
        ValkyrieType::TraitObject(object) => {
            for argument in &mut object.type_arguments {
                propagate_declared_generic_binders(argument, generics);
            }
        }
        ValkyrieType::Row(row) => {
            for method in &mut row.methods {
                for param in &mut method.params {
                    propagate_declared_generic_binders(param, generics);
                }
                propagate_declared_generic_binders(&mut method.return_type, generics);
            }
        }
        ValkyrieType::TypeLambda(lambda) => propagate_declared_generic_binders(&mut lambda.body, generics),
        _ => {}
    }
}

pub(crate) fn propagate_generic_binders_in_block(block: &mut HirBlock, generics: &[GenericType]) {
    for statement in &mut block.statements {
        match &mut statement.kind {
            HirStatementKind::Let { ty, initializer, .. } => {
                if let Some(ty) = ty {
                    propagate_declared_generic_binders(ty, generics);
                }
                if let Some(initializer) = initializer {
                    propagate_generic_binders_in_expr(initializer, generics);
                }
            }
            HirStatementKind::Expr(expr) => propagate_generic_binders_in_expr(expr, generics),
        }
    }
    if let Some(expr) = &mut block.expr {
        propagate_generic_binders_in_expr(expr, generics);
    }
}

pub(crate) fn propagate_generic_binders_in_expr(expr: &mut HirExpr, generics: &[GenericType]) {
    match &mut expr.kind {
        HirExprKind::Cast { ty, expr: inner, .. } => {
            propagate_declared_generic_binders(ty, generics);
            propagate_generic_binders_in_expr(inner, generics);
        }
        HirExprKind::Lambda { params, return_type, body, captures, hoisted, .. } => {
            for param in params {
                propagate_declared_generic_binders(&mut param.ty, generics);
            }
            propagate_declared_generic_binders(return_type, generics);
            for capture in captures {
                propagate_declared_generic_binders(&mut capture.ty, generics);
            }
            // 环境聚合的实参以 `Named` 占位回填，此处按**外层 callable** 的泛型
            // 绑定 bearer，保证单态化代入的是同一个 bearer。
            if let Some(environment) = hoisted.as_mut().and_then(|identity| identity.environment.as_mut()) {
                if let Some(ty) = environment.ty.as_mut() {
                    propagate_declared_generic_binders(ty, generics);
                }
            }
            propagate_generic_binders_in_block(body, generics);
        }
        HirExprKind::TryScope { result_type, body, .. } => {
            if let Some(ty) = result_type {
                propagate_declared_generic_binders(ty, generics);
            }
            propagate_generic_binders_in_block(body, generics);
        }
        HirExprKind::Block(block) => propagate_generic_binders_in_block(block, generics),
        HirExprKind::If { condition, then_branch, else_branch, .. } => {
            propagate_generic_binders_in_expr(condition, generics);
            propagate_generic_binders_in_block(then_branch, generics);
            if let Some(branch) = else_branch {
                propagate_generic_binders_in_block(branch, generics);
            }
        }
        HirExprKind::Return(Some(value))
        | HirExprKind::Yield(Some(value))
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::Raise(value)
        | HirExprKind::Resume(value) => propagate_generic_binders_in_expr(value, generics),
        HirExprKind::TryPropagate { inner, .. } => propagate_generic_binders_in_expr(inner, generics),
        HirExprKind::Call { callee, args, .. } => {
            propagate_generic_binders_in_expr(callee, generics);
            for arg in args {
                propagate_generic_binders_in_expr(&mut arg.value, generics);
            }
        }
        HirExprKind::Construct { args, .. } => {
            for arg in args {
                propagate_generic_binders_in_expr(arg, generics);
            }
        }
        HirExprKind::FieldAccess { object, .. } | HirExprKind::FieldInit { value: object, .. } => {
            propagate_generic_binders_in_expr(object, generics);
        }
        HirExprKind::StoreField { object, value, .. } => {
            propagate_generic_binders_in_expr(object, generics);
            propagate_generic_binders_in_expr(value, generics);
        }
        HirExprKind::GenericApply { callee, arguments, .. } => {
            propagate_generic_binders_in_expr(callee, generics);
            for argument in arguments {
                propagate_declared_generic_binders(argument, generics);
            }
        }
        HirExprKind::ArrayNew { length, .. } => propagate_generic_binders_in_expr(length, generics),
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } => {
            for item in items {
                propagate_generic_binders_in_expr(item, generics);
            }
        }
        HirExprKind::IfLet { scrutinee, then_branch, else_branch, .. } => {
            propagate_generic_binders_in_expr(scrutinee, generics);
            propagate_generic_binders_in_block(then_branch, generics);
            if let Some(branch) = else_branch {
                propagate_generic_binders_in_block(branch, generics);
            }
        }
        HirExprKind::Match { scrutinee, arms, .. } | HirExprKind::Case { scrutinee, arms, .. } => {
            propagate_generic_binders_in_expr(scrutinee, generics);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    propagate_generic_binders_in_expr(guard, generics);
                }
                propagate_generic_binders_in_expr(&mut arm.body, generics);
            }
        }
        HirExprKind::Loop { iterator, body, condition, .. } => {
            if let Some(iterator) = iterator {
                propagate_generic_binders_in_expr(iterator, generics);
            }
            if let Some(condition) = condition {
                propagate_generic_binders_in_expr(condition, generics);
            }
            propagate_generic_binders_in_block(body, generics);
        }
        HirExprKind::Catch { expr, arms, .. } => {
            propagate_generic_binders_in_expr(expr, generics);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    propagate_generic_binders_in_expr(guard, generics);
                }
                propagate_generic_binders_in_expr(&mut arm.body, generics);
            }
        }
        HirExprKind::AnonymousClass { fields, .. } => {
            for (_, value) in fields {
                propagate_generic_binders_in_expr(value, generics);
            }
        }
        _ => {}
    }
}

pub(crate) fn freeze_function_generic_binders(function: &mut HirFunction) {
    let Some(owner) = function.declaration
    else {
        return;
    };
    freeze_generic_binders(&mut function.generics, owner);
    for param in &mut function.params {
        propagate_declared_generic_binders(&mut param.ty, &function.generics);
    }
    propagate_declared_generic_binders(&mut function.return_type, &function.generics);
    for constraint in &mut function.where_constraints {
        propagate_declared_generic_binders(&mut constraint.target, &function.generics);
        for bound in &mut constraint.bounds {
            for argument in &mut bound.type_arguments {
                propagate_declared_generic_binders(argument, &function.generics);
            }
            for equation in &mut bound.associated_types {
                propagate_declared_generic_binders(&mut equation.ty, &function.generics);
            }
        }
    }
    propagate_generic_binders_in_block(&mut function.body, &function.generics);
}

/// 冻结成员自身的泛型后，把**复用同一声明的 owner 泛型**一并写进成员的所有类型位。
///
/// 成员（方法/构造器/默认实现）自身可能没有泛型参数，其签名里的 `T` 来自 owner
/// 声明。这类 binder 必须在声明冻结期与 owner 声明一起写定，调用点只读不重建。
pub(crate) fn freeze_member_generic_binders(function: &mut HirFunction, owner_generics: &[GenericType]) {
    freeze_function_generic_binders(function);
    if owner_generics.is_empty() {
        return;
    }
    propagate_declared_generic_binders(&mut function.return_type, owner_generics);
    for param in &mut function.params {
        propagate_declared_generic_binders(&mut param.ty, owner_generics);
    }
    propagate_generic_binders_in_where_constraints(&mut function.where_constraints, owner_generics);
    propagate_generic_binders_in_block(&mut function.body, owner_generics);
}

pub(crate) fn freeze_enum_generic_binders(enum_definition: &mut HirEnum) {
    let Some(owner) = enum_definition.declaration
    else {
        return;
    };
    freeze_generic_binders(&mut enum_definition.generics, owner);
    for variant in &mut enum_definition.variants {
        for field in &mut variant.fields {
            propagate_declared_generic_binders(&mut field.ty, &enum_definition.generics);
        }
        if let Some(result_type) = &mut variant.result_type {
            propagate_declared_generic_binders(result_type, &enum_definition.generics);
        }
    }
}

pub(crate) fn freeze_struct_generic_binders(structure: &mut HirStruct) {
    let Some(owner) = structure.constructor_declaration
    else {
        return;
    };
    freeze_generic_binders(&mut structure.generics, owner);
    let generics = structure.generics.clone();
    for field in &mut structure.fields {
        propagate_declared_generic_binders(&mut field.ty, &generics);
    }
    for method in &mut structure.methods {
        freeze_member_generic_binders(method, &generics);
    }
}

pub(crate) fn freeze_trait_generic_binders(trait_definition: &mut HirTrait) {
    let Some(owner) = trait_definition.declaration
    else {
        return;
    };
    freeze_generic_binders(&mut trait_definition.generics, owner);
    let generics = trait_definition.generics.clone();
    for associated in &mut trait_definition.associated_types {
        freeze_generic_binders(&mut associated.type_params, owner);
        for bound in &mut associated.bounds {
            propagate_declared_generic_binders(bound, &generics);
        }
        if let Some(default) = &mut associated.default {
            propagate_declared_generic_binders(default, &generics);
        }
    }
    for method in trait_definition.methods.iter_mut().chain(trait_definition.default_methods.iter_mut()) {
        freeze_member_generic_binders(method, &generics);
    }
}

/// 声明冻结期写定 impl 成员的泛型 binder。
///
/// 三类 callable 的 `where` 目标都在这里唯一确定：
/// - 自由函数：`freeze_function_generic_binders`（function binder）；
/// - impl 成员：impl 头显式泛型 + owner 声明泛型 + trait 声明泛型（owner binder）；
/// - trait 成员：trait 声明泛型（trait binder）。
///
/// 调用点只读取该结果，不再按 receiver 实参回填。
pub(crate) fn freeze_impl_generic_binders(implementation: &mut HirImpl, module: &HirModule) -> Result<(), HirError> {
    let owner_generics = owner_generics_for_target(module, &implementation.target);
    let trait_generics = implementation
        .trait_declaration
        .map(|declaration| trait_generics_by_declaration(module, declaration))
        .unwrap_or_default();
    // impl 头部显式泛型按位置对齐 owner 声明参数；名字命中只作为次选，
    // 不按短名新铸身份。
    for (ordinal, generic) in implementation.generics.iter_mut().enumerate() {
        if generic.binder.is_some() {
            continue;
        }
        let declared = owner_generics
            .get(ordinal)
            .or_else(|| owner_generics.iter().find(|item| item.name == generic.name))
            .or_else(|| trait_generics.get(ordinal))
            .or_else(|| trait_generics.iter().find(|item| item.name == generic.name));
        if let Some(declared) = declared {
            generic.binder = declared.binder;
            if generic.bounds.is_empty() {
                generic.bounds = declared.bounds.clone();
            }
        }
    }
    let mut context = implementation.generics.clone();
    for declared in owner_generics.iter().chain(trait_generics.iter()) {
        if !context.iter().any(|item| item.name == declared.name) {
            context.push(declared.clone());
        }
    }
    // impl target 里裸写的名字只有两种合法来源：impl 头显式声明的泛型，
    // 或 owner 声明同名的泛型参数；其余名字保持未绑定形态并结构化失败。
    propagate_declared_generic_binders(&mut implementation.target, &context);
    propagate_generic_binders_in_where_constraints(&mut implementation.where_constraints, &context);
    for argument in &mut implementation.trait_arguments {
        propagate_declared_generic_binders(argument, &context);
    }
    for associated in &mut implementation.associated_type_impls {
        propagate_declared_generic_binders(&mut associated.concrete_type, &context);
        for argument in &mut associated.type_args {
            propagate_declared_generic_binders(argument, &context);
        }
    }
    for method in &mut implementation.methods {
        freeze_member_generic_binders(method, &context);
    }
    if let Some((member, generic)) = unresolved_impl_generic(implementation) {
        let declaration = crate::compiler::hir::overload::r#trait::impl_owner_identifier(module, &implementation.target)
            .map(|name| name.to_string())
            .unwrap_or_else(|| "<unresolved impl target>".to_string());
        return Err(HirError::ImplGenericBinderUnresolved {
            declaration,
            member,
            generic: generic.to_string(),
            source: plan_source(),
            span: plan_span(),
        });
    }
    Ok(())
}

fn owner_generics_for_target(module: &HirModule, target: &ValkyrieType) -> Vec<GenericType> {
    let declaration = match target {
        ValkyrieType::Nominal { declaration, .. } => Some(*declaration),
        ValkyrieType::Array(_) => super::array_owner_declaration(module).ok().flatten(),
        _ => None,
    };
    declaration.map(|declaration| super::lookup_owner_generics_by_declaration(module, declaration)).unwrap_or_default()
}

fn trait_generics_by_declaration(module: &HirModule, declaration: nyar_types::ItemId) -> Vec<GenericType> {
    if let Some(item) = module.traits.iter().find(|item| item.declaration == Some(declaration)) {
        return item.generics.clone();
    }
    for export in &module.imported_semantic_exports {
        if let Some(item) = export.traits.iter().find(|item| item.declaration == Some(declaration)) {
            return item.generics.clone();
        }
    }
    for submodule in &module.submodules {
        let nested = trait_generics_by_declaration(submodule, declaration);
        if !nested.is_empty() {
            return nested;
        }
    }
    Vec::new()
}

/// impl 成员声明面残留的未绑定泛型（binder 为空）必须点名失败。
///
/// 只看成员声明面（where / 关联实现 / 形参 / 返回类型）：impl target 上未绑定的
/// 头部实参不构成成员签名，它只是不可求证据的 impl 身份，由候选匹配阶段淘汰；
/// 体内类型位走同一份传播，若残留会在后续单态化阶段以既有结构化错误暴露。
fn unresolved_impl_generic(implementation: &HirImpl) -> Option<(String, Identifier)> {
    fn find_in_constraints(constraints: &[crate::compiler::types::hir::HirWhereConstraint]) -> Option<&Identifier> {
        for constraint in constraints {
            if let Some(name) = unfrozen_generic_name(&constraint.target) {
                return Some(name);
            }
            for bound in &constraint.bounds {
                for argument in &bound.type_arguments {
                    if let Some(name) = unfrozen_generic_name(argument) {
                        return Some(name);
                    }
                }
                for equation in &bound.associated_types {
                    if let Some(name) = unfrozen_generic_name(&equation.ty) {
                        return Some(name);
                    }
                }
            }
        }
        None
    }
    if let Some(name) = find_in_constraints(&implementation.where_constraints) {
        return Some(("<impl where>".to_string(), name.clone()));
    }
    for argument in &implementation.trait_arguments {
        if let Some(name) = unfrozen_generic_name(argument) {
            return Some(("<impl trait arguments>".to_string(), name.clone()));
        }
    }
    for associated in &implementation.associated_type_impls {
        if let Some(name) = unfrozen_generic_name(&associated.concrete_type) {
            return Some((associated.name.to_string(), name.clone()));
        }
    }
    for method in &implementation.methods {
        if let Some(name) = unfrozen_generic_name(&method.return_type) {
            return Some((method.name.to_string(), name.clone()));
        }
        for param in &method.params {
            if let Some(name) = unfrozen_generic_name(&param.ty) {
                return Some((method.name.to_string(), name.clone()));
            }
        }
        if let Some(name) = find_in_constraints(&method.where_constraints) {
            return Some((method.name.to_string(), name.clone()));
        }
    }
    None
}

fn unfrozen_generic_name(ty: &ValkyrieType) -> Option<&Identifier> {
    match ty {
        ValkyrieType::Generic(generic) if generic.binder.is_none() => Some(&generic.name),
        ValkyrieType::Nominal { arguments, .. } | ValkyrieType::Apply(_, arguments) | ValkyrieType::Tuple(arguments) => {
            arguments.iter().find_map(unfrozen_generic_name)
        }
        ValkyrieType::Union(arguments) | ValkyrieType::Intersection(arguments) => arguments.iter().find_map(unfrozen_generic_name),
        ValkyrieType::Array(inner) | ValkyrieType::Nullable(inner) | ValkyrieType::FixedArray { element: inner, .. } => unfrozen_generic_name(inner),
        ValkyrieType::Function(function) => {
            function.params.iter().find_map(unfrozen_generic_name).or_else(|| unfrozen_generic_name(&function.return_type))
        }
        ValkyrieType::Associated(associated) => {
            unfrozen_generic_name(&associated.base).or_else(|| associated.type_arguments.iter().find_map(unfrozen_generic_name))
        }
        ValkyrieType::TraitObject(object) => object.type_arguments.iter().find_map(unfrozen_generic_name),
        ValkyrieType::Row(row) => row.methods.iter().find_map(|method| {
            method.params.iter().find_map(unfrozen_generic_name).or_else(|| unfrozen_generic_name(&method.return_type))
        }),
        ValkyrieType::TypeLambda(lambda) => unfrozen_generic_name(&lambda.body),
        ValkyrieType::LabeledTuple(fields) => fields.iter().find_map(|field| unfrozen_generic_name(&field.ty)),
        _ => None,
    }
}

/// 冻结整棵模块树的泛型 binder。
///
/// 两相执行：先把所有 owner 声明的泛型列表写出身份（含子模块与跨引用），
/// 再让成员按 owner 上下文写定。反过来会让 impl 在 owner 尚未冻结时读到空 binder。
pub(crate) fn freeze_module_generic_binders(module: &mut HirModule) -> Result<(), HirError> {
    freeze_owner_generic_lists(module);
    freeze_member_generic_binders_in_module(module)
}

fn freeze_owner_generic_lists(module: &mut HirModule) {
    for structure in &mut module.structs {
        if let Some(owner) = structure.constructor_declaration {
            freeze_generic_binders(&mut structure.generics, owner);
        }
    }
    for enum_definition in &mut module.enums {
        if let Some(owner) = enum_definition.declaration {
            freeze_generic_binders(&mut enum_definition.generics, owner);
        }
    }
    for trait_definition in &mut module.traits {
        if let Some(owner) = trait_definition.declaration {
            freeze_generic_binders(&mut trait_definition.generics, owner);
            for associated in &mut trait_definition.associated_types {
                freeze_generic_binders(&mut associated.type_params, owner);
            }
        }
    }
    for submodule in &mut module.submodules {
        freeze_owner_generic_lists(submodule);
    }
}

fn freeze_member_generic_binders_in_module(module: &mut HirModule) -> Result<(), HirError> {
    for function in &mut module.functions {
        freeze_function_generic_binders(function);
    }
    for structure in &mut module.structs {
        freeze_struct_generic_binders(structure);
    }
    for enum_definition in &mut module.enums {
        freeze_enum_generic_binders(enum_definition);
    }
    for singleton in &mut module.singletons {
        let generics = singleton.generics.clone();
        for field in &mut singleton.fields {
            propagate_declared_generic_binders(&mut field.ty, &generics);
        }
        for method in singleton
            .methods
            .iter_mut()
            .chain(singleton.constructor.iter_mut().map(Box::as_mut))
            .chain(singleton.finalizer.iter_mut().map(Box::as_mut))
        {
            freeze_member_generic_binders(method, &generics);
        }
    }
    for trait_definition in &mut module.traits {
        freeze_trait_generic_binders(trait_definition);
    }
    // impl 成员的 owner 泛型查找需要整份模块的声明表，先把 impl 列表移出再写回。
    let mut impls = std::mem::take(&mut module.impls);
    let result = impls.iter_mut().try_for_each(|implementation| freeze_impl_generic_binders(implementation, module));
    module.impls = impls;
    result?;
    for widget in &mut module.widgets {
        for method in &mut widget.methods {
            freeze_function_generic_binders(method);
        }
    }
    for submodule in &mut module.submodules {
        freeze_member_generic_binders_in_module(submodule)?;
    }
    Ok(())
}

pub(crate) fn lower_trait_method(mut method: HirFunction, associated_names: &std::collections::BTreeSet<Identifier>) -> HirFunction {
    let generic_names = method.generics.iter().map(|generic| generic.name.clone()).collect::<std::collections::BTreeSet<_>>();
    for param in &mut method.params {
        param.ty = lower_trait_associated_type_references(&param.ty, associated_names, &generic_names);
    }
    method.return_type = lower_trait_associated_type_references(&method.return_type, associated_names, &generic_names);
    for constraint in &mut method.where_constraints {
        constraint.target = lower_trait_associated_type_references(&constraint.target, associated_names, &generic_names);
        for bound in &mut constraint.bounds {
            for argument in &mut bound.type_arguments {
                *argument = lower_trait_associated_type_references(argument, associated_names, &generic_names);
            }
            for equation in &mut bound.associated_types {
                equation.ty = lower_trait_associated_type_references(&equation.ty, associated_names, &generic_names);
            }
        }
    }
    method
}

pub(crate) fn lower_trait_associated_type_references(
    ty: &ValkyrieType,
    associated_names: &std::collections::BTreeSet<Identifier>,
    generic_names: &std::collections::BTreeSet<Identifier>,
) -> ValkyrieType {
    match ty {
        ValkyrieType::Named(name) if associated_names.contains(name) && !generic_names.contains(name) => {
            ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
                base: ValkyrieType::SelfType,
                name: name.clone(),
                type_arguments: Vec::new(),
            }))
        }
        ValkyrieType::Apply(base, arguments) => ValkyrieType::Apply(
            Box::new(lower_trait_associated_type_references(base, associated_names, generic_names)),
            arguments.iter().map(|argument| lower_trait_associated_type_references(argument, associated_names, generic_names)).collect(),
        ),
        ValkyrieType::Array(element) => {
            ValkyrieType::Array(Box::new(lower_trait_associated_type_references(element, associated_names, generic_names)))
        }
        ValkyrieType::FixedArray { element, length } => ValkyrieType::FixedArray {
            element: Box::new(lower_trait_associated_type_references(element, associated_names, generic_names)),
            length: *length,
        },
        ValkyrieType::Nullable(inner) => {
            ValkyrieType::Nullable(Box::new(lower_trait_associated_type_references(inner, associated_names, generic_names)))
        }
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(
            items.iter().map(|item| lower_trait_associated_type_references(item, associated_names, generic_names)).collect(),
        ),
        ValkyrieType::Union(items) => ValkyrieType::Union(
            items.iter().map(|item| lower_trait_associated_type_references(item, associated_names, generic_names)).collect(),
        ),
        ValkyrieType::Intersection(items) => ValkyrieType::Intersection(
            items.iter().map(|item| lower_trait_associated_type_references(item, associated_names, generic_names)).collect(),
        ),
        ValkyrieType::Function(function) => ValkyrieType::Function(Box::new(crate::compiler::types::hir::FunctionType {
            params: function
                .params
                .iter()
                .map(|param| lower_trait_associated_type_references(param, associated_names, generic_names))
                .collect(),
            return_type: lower_trait_associated_type_references(&function.return_type, associated_names, generic_names),
        })),
        ValkyrieType::Associated(associated) => ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
            base: lower_trait_associated_type_references(&associated.base, associated_names, generic_names),
            name: associated.name.clone(),
            type_arguments: associated
                .type_arguments
                .iter()
                .map(|argument| lower_trait_associated_type_references(argument, associated_names, generic_names))
                .collect(),
        })),
        other => other.clone(),
    }
}
