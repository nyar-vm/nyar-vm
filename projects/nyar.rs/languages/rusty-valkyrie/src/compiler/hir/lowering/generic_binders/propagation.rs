use crate::compiler::types::{
    Identifier,
    hir::{
        GenericBinderId, GenericType, HirBlock, HirEnum, HirExpr, HirExprKind, HirFunction, HirImpl, HirModule, HirStatementKind, HirStruct,
        HirTrait, ValkyrieType,
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
        HirExprKind::Lambda { params, return_type, body, .. } => {
            for param in params {
                propagate_declared_generic_binders(&mut param.ty, generics);
            }
            propagate_declared_generic_binders(return_type, generics);
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
    for field in &mut structure.fields {
        propagate_declared_generic_binders(&mut field.ty, &structure.generics);
    }
    for method in &mut structure.methods {
        freeze_function_generic_binders(method);
    }
    for property in &mut structure.properties {
        if let Some(getter) = &mut property.getter {
            freeze_function_generic_binders(getter);
        }
        if let Some(setter) = &mut property.setter {
            freeze_function_generic_binders(setter);
        }
    }
}

pub(crate) fn freeze_trait_generic_binders(trait_definition: &mut HirTrait) {
    let Some(owner) = trait_definition.declaration
    else {
        return;
    };
    freeze_generic_binders(&mut trait_definition.generics, owner);
    for associated in &mut trait_definition.associated_types {
        freeze_generic_binders(&mut associated.type_params, owner);
        for bound in &mut associated.bounds {
            propagate_declared_generic_binders(bound, &trait_definition.generics);
        }
        if let Some(default) = &mut associated.default {
            propagate_declared_generic_binders(default, &trait_definition.generics);
        }
    }
    for method in trait_definition.methods.iter_mut().chain(trait_definition.default_methods.iter_mut()) {
        freeze_function_generic_binders(method);
    }
}

pub(crate) fn freeze_impl_generic_binders(implementation: &mut HirImpl) {
    propagate_declared_generic_binders(&mut implementation.target, &implementation.generics);
    for method in &mut implementation.methods {
        freeze_function_generic_binders(method);
    }
}

pub(crate) fn freeze_module_generic_binders(module: &mut HirModule) {
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
        for method in singleton
            .methods
            .iter_mut()
            .chain(singleton.constructor.iter_mut().map(Box::as_mut))
            .chain(singleton.finalizer.iter_mut().map(Box::as_mut))
        {
            freeze_function_generic_binders(method);
        }
    }
    for trait_definition in &mut module.traits {
        freeze_trait_generic_binders(trait_definition);
    }
    for implementation in &mut module.impls {
        freeze_impl_generic_binders(implementation);
    }
    for widget in &mut module.widgets {
        for method in &mut widget.methods {
            freeze_function_generic_binders(method);
        }
    }
    for submodule in &mut module.submodules {
        freeze_module_generic_binders(submodule);
    }
}

/// Trait 成员以 `;` 结束时 parser 常给出空块而非 `body: None`，需回收为纯签名。
pub(crate) fn finalize_trait_method_signature(mut method: HirFunction) -> HirFunction {
    if method.body.statements.is_empty() && method.body.expr.is_none() {
        method.is_abstract = true;
    }
    method
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
