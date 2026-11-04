//! 闭包 lambda 提升：冻结合成函数实例与 per-lambda 捕获环境聚合身份。
//!
//! 闭包值由冻结 callable 实例（代码指针）与捕获环境聚合共同组成，二者必须在
//! 同一 HIR 冻结事实中产生。本模块只铸造并回填身份，不引入新的语言类型，也不做
//! 目标侧环境传递；MIR 只消费这里冻结的 `ItemInstanceId` / `FieldId`。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::compiler::types::{
    Identifier, NamePath, SourceSpan,
    hir::{
        GenericType, HirBlock, HirCapture, HirDocumentation, HirExpr, HirExprKind, HirExtractorPattern, HirField, HirFunction,
        HirIdentifier, HirLambdaEnvironment, HirLambdaIdentity, HirMatchArm, HirModule, HirParam, HirPattern, HirStatement,
        HirStatementKind, HirStruct, HirVisibility, HirWhereConstraint, ValkyrieType,
    },
};

use super::CaptureAnalyzer;

static LAMBDA_COUNTER: AtomicU32 = AtomicU32::new(0);

const ENV_PARAMETER: &str = "__nyar_env";

#[derive(Debug, Clone)]
struct LocalBinding {
    ty: ValkyrieType,
    is_mutable: bool,
}

struct HoistContext {
    namespace: Vec<Identifier>,
    /// 当前 callable 的泛型参数：合成函数与环境聚合必须继承，否则捕获到的
    /// `T` 会变成自由泛型，泛型 owner 的每个实例都会共享同一环境类型。
    owner_generics: Vec<GenericType>,
    /// 当前 callable 的 `where` 假设：lambda 是函数体词法域内的闭包，体内调用
    /// 只能借助外层已成立的 witness 证据求解（如 `T: Equal & Hash` 才允许
    /// `result.insert(value)`），合成函数不继承这组假设就会被误判为无解。
    owner_where_constraints: Vec<HirWhereConstraint>,
    /// 当前 impl 的冻结目标类型；捕获变量的 `SelfType` 必须落成具体名义类型，
    /// 否则环境字段没有接收者类型，lambda 体内的成员调用无法解析。
    owner_self_type: Option<ValkyrieType>,
    pending_functions: Vec<HirFunction>,
    pending_structs: Vec<HirStruct>,
}

/// 提升模块内全部 lambda 表达式，把合成函数与环境结构登记为普通 HIR 项。
///
/// 必须在 `register_function_declarations` / `register_field_declarations` 之前调用。
pub fn hoist_lambdas(module: &mut HirModule) {
    for submodule in &mut module.submodules {
        hoist_lambdas(submodule);
    }

    let mut context = HoistContext {
        namespace: module.name.parts().to_vec(),
        owner_generics: Vec::new(),
        owner_where_constraints: Vec::new(),
        owner_self_type: None,
        pending_functions: Vec::new(),
        pending_structs: Vec::new(),
    };
    // impl 头部的 `imply HashSet<T>` 泛型挂在**目标类型**上而不是 `implementation.generics`；
    // 合成函数与环境必须继承目标声明的泛型，否则每个 owner 实例会共享同一个非泛型载体。
    let struct_generics = module
        .structs
        .iter()
        .map(|structure| (structure.name.clone(), structure.generics.clone()))
        .collect::<BTreeMap<_, _>>();

    for function in &mut module.functions {
        context.owner_generics = function.generics.clone();
        context.owner_where_constraints = function.where_constraints.clone();
        context.owner_self_type = None;
        hoist_function(function, &mut context);
    }
    for structure in &mut module.structs {
        let class_name = structure.name.clone();
        let owner_generics = structure.generics.clone();
        for method in &mut structure.methods {
            context.owner_generics = merge_generics(&owner_generics, &method.generics);
            context.owner_where_constraints = method.where_constraints.clone();
            context.owner_self_type = Some(ValkyrieType::Named(class_name.clone()));
            let mut locals = locals_from_params(&method.params);
            locals.insert("self".to_string(), LocalBinding { ty: ValkyrieType::Named(class_name.clone()), is_mutable: true });
            hoist_function_body(method, &mut context, locals);
        }
    }
    for implementation in &mut module.impls {
        let target_generics = match &implementation.target {
            ValkyrieType::Apply(base, _) => match base.as_ref() {
                ValkyrieType::Named(name) => struct_generics.get(name).cloned().unwrap_or_default(),
                _ => Vec::new(),
            },
            ValkyrieType::Named(name) => struct_generics.get(name).cloned().unwrap_or_default(),
            _ => Vec::new(),
        };
        let impl_generics = merge_generics(&target_generics, &implementation.generics);
        for method in &mut implementation.methods {
            context.owner_generics = merge_generics(&impl_generics, &method.generics);
            context.owner_where_constraints = method.where_constraints.clone();
            context.owner_self_type = Some(implementation.target.clone());
            let mut locals = locals_from_params(&method.params);
            locals.insert("self".to_string(), LocalBinding { ty: ValkyrieType::SelfType, is_mutable: true });
            hoist_function_body(method, &mut context, locals);
        }
    }
    for trait_declaration in &mut module.traits {
        let owner_generics = trait_declaration.generics.clone();
        for method in trait_declaration.methods.iter_mut().chain(trait_declaration.default_methods.iter_mut()) {
            context.owner_generics = merge_generics(&owner_generics, &method.generics);
            context.owner_where_constraints = method.where_constraints.clone();
            context.owner_self_type = None;
            let mut locals = locals_from_params(&method.params);
            locals.insert("self".to_string(), LocalBinding { ty: ValkyrieType::SelfType, is_mutable: true });
            hoist_function_body(method, &mut context, locals);
        }
    }
    for singleton in &mut module.singletons {
        context.owner_generics.clear();
        context.owner_where_constraints.clear();
        context.owner_self_type = None;
        for method in &mut singleton.methods {
            hoist_function(method, &mut context);
        }
        if let Some(constructor) = &mut singleton.constructor {
            hoist_function(constructor, &mut context);
        }
        if let Some(finalizer) = &mut singleton.finalizer {
            hoist_function(finalizer, &mut context);
        }
    }
    for widget in &mut module.widgets {
        let owner_generics = widget.generics.clone();
        for method in &mut widget.methods {
            context.owner_generics = merge_generics(&owner_generics, &method.generics);
            context.owner_where_constraints = method.where_constraints.clone();
            context.owner_self_type = None;
            let mut locals = locals_from_params(&method.params);
            locals.insert("self".to_string(), LocalBinding { ty: ValkyrieType::SelfType, is_mutable: true });
            hoist_function_body(method, &mut context, locals);
        }
    }
    for type_function in &mut module.type_functions {
        context.owner_generics = type_function.generics.clone();
        context.owner_where_constraints = type_function.where_constraints.clone();
        context.owner_self_type = None;
        let mut locals = BTreeMap::new();
        hoist_block(&mut type_function.body, &mut context, &mut locals);
    }
    context.owner_generics.clear();
    context.owner_where_constraints.clear();
    context.owner_self_type = None;
    for statement in &mut module.statements {
        let mut locals = BTreeMap::new();
        hoist_statement(statement, &mut context, &mut locals);
    }

    module.functions.extend(context.pending_functions);
    module.structs.extend(context.pending_structs);
}

/// 合并泛型参数：owner 声明在前，lambda 自身声明只补未出现过的名字。
fn merge_generics(owner: &[GenericType], local: &[GenericType]) -> Vec<GenericType> {
    let mut merged = owner.to_vec();
    for generic in local {
        if !merged.iter().any(|existing| existing.name == generic.name) {
            merged.push(generic.clone());
        }
    }
    merged
}

fn hoist_function(function: &mut HirFunction, context: &mut HoistContext) {
    let mut locals = BTreeMap::new();
    for param in &mut function.params {
        if let Some(default) = &mut param.default {
            hoist_expr(default, context, &locals);
        }
        locals.insert(param.name.name.to_string(), LocalBinding { ty: param.ty.clone(), is_mutable: param.is_mutable });
    }
    hoist_block(&mut function.body, context, &mut locals);
}

fn hoist_function_body(function: &mut HirFunction, context: &mut HoistContext, mut locals: BTreeMap<String, LocalBinding>) {
    hoist_block(&mut function.body, context, &mut locals);
}

fn locals_from_params(params: &[HirParam]) -> BTreeMap<String, LocalBinding> {
    params.iter().map(|param| (param.name.name.to_string(), LocalBinding { ty: param.ty.clone(), is_mutable: param.is_mutable })).collect()
}

fn hoist_block(block: &mut HirBlock, context: &mut HoistContext, locals: &mut BTreeMap<String, LocalBinding>) {
    for statement in &mut block.statements {
        hoist_statement(statement, context, locals);
    }
    if let Some(expr) = &mut block.expr {
        hoist_expr(expr, context, locals);
    }
}

fn hoist_statement(statement: &mut HirStatement, context: &mut HoistContext, locals: &mut BTreeMap<String, LocalBinding>) {
    match &mut statement.kind {
        HirStatementKind::Let { is_mutable, pattern, ty, initializer, .. } => {
            if let Some(initializer) = initializer {
                hoist_expr(initializer, context, locals);
            }
            let binding_ty = ty.clone().or_else(|| initializer.as_ref().and_then(|init| infer_expr_type(init, locals)));
            let binding_ty = binding_ty.unwrap_or(ValkyrieType::AutoType);
            bind_pattern_names(pattern, &binding_ty, *is_mutable, locals);
        }
        HirStatementKind::Expr(expr) => hoist_expr(expr, context, locals),
    }
}

fn hoist_expr(expr: &mut HirExpr, context: &mut HoistContext, locals: &BTreeMap<String, LocalBinding>) {
    if let HirExprKind::Lambda { generics, params, return_type, body, captures, hoisted } = &mut expr.kind {
        if hoisted.is_none() {
            hoist_lambda(expr.span.clone(), generics, params, return_type, body, captures, hoisted, context, locals);
        }
        return;
    }

    match &mut expr.kind {
        HirExprKind::Call { callee, args, .. } => {
            hoist_expr(callee, context, locals);
            for arg in args {
                hoist_expr(&mut arg.value, context, locals);
            }
        }
        HirExprKind::Construct { args, .. } => {
            for arg in args {
                hoist_expr(arg, context, locals);
            }
        }
        HirExprKind::FieldInit { value, .. } => hoist_expr(value, context, locals),
        HirExprKind::ArrayNew { length, .. } => hoist_expr(length, context, locals),
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } => {
            for item in items {
                hoist_expr(item, context, locals);
            }
        }
        HirExprKind::CollectionInit { items, .. } => {
            for item in items {
                hoist_expr(item, context, locals);
            }
        }
        HirExprKind::FieldAccess { object, .. } => hoist_expr(object, context, locals),
        HirExprKind::StoreField { object, value, .. } => {
            hoist_expr(object, context, locals);
            hoist_expr(value, context, locals);
        }
        HirExprKind::GenericApply { callee, .. } => hoist_expr(callee, context, locals),
        HirExprKind::Block(block) => hoist_block(block, context, &mut locals.clone()),
        HirExprKind::AnonymousClass { fields, .. } => {
            for (_, value) in fields {
                hoist_expr(value, context, locals);
            }
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            hoist_expr(condition, context, locals);
            hoist_block(then_branch, context, &mut locals.clone());
            if let Some(else_branch) = else_branch {
                hoist_block(else_branch, context, &mut locals.clone());
            }
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            hoist_expr(scrutinee, context, locals);
            let mut then_locals = locals.clone();
            bind_pattern_names(pattern, &ValkyrieType::AutoType, false, &mut then_locals);
            hoist_block(then_branch, context, &mut then_locals);
            if let Some(else_branch) = else_branch {
                hoist_block(else_branch, context, &mut locals.clone());
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            hoist_expr(scrutinee, context, locals);
            for arm in arms {
                hoist_match_arm(arm, context, locals);
            }
        }
        HirExprKind::Loop { pattern, iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                hoist_expr(iterator, context, locals);
            }
            if let Some(condition) = condition {
                hoist_expr(condition, context, locals);
            }
            let mut body_locals = locals.clone();
            if let Some(pattern) = pattern {
                bind_pattern_names(pattern, &ValkyrieType::AutoType, false, &mut body_locals);
            }
            hoist_block(body, context, &mut body_locals);
        }
        HirExprKind::Return(Some(value))
        | HirExprKind::Yield(Some(value))
        | HirExprKind::Resume(value)
        | HirExprKind::TryPropagate { inner: value, .. }
        | HirExprKind::Raise(value)
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::Cast { expr: value, .. } => hoist_expr(value, context, locals),
        HirExprKind::Assign { value, .. } => hoist_expr(value, context, locals),
        HirExprKind::Break { expr: Some(value), .. } => hoist_expr(value, context, locals),
        HirExprKind::Catch { expr: value, arms } => {
            hoist_expr(value, context, locals);
            for arm in arms {
                hoist_match_arm(arm, context, locals);
            }
        }
        HirExprKind::TryScope { body, .. } => hoist_block(body, context, &mut locals.clone()),
        HirExprKind::With { base, updates } => {
            hoist_expr(base, context, locals);
            for (_, value) in updates {
                hoist_expr(value, context, locals);
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                hoist_expr(arg, context, locals);
            }
        }
        _ => {}
    }
}

fn hoist_match_arm(arm: &mut HirMatchArm, context: &mut HoistContext, locals: &BTreeMap<String, LocalBinding>) {
    let mut arm_locals = locals.clone();
    bind_pattern_names(&arm.pattern, &ValkyrieType::AutoType, false, &mut arm_locals);
    if let Some(guard) = &mut arm.guard {
        hoist_expr(guard, context, &arm_locals);
    }
    hoist_expr(&mut arm.body, context, &arm_locals);
}

#[allow(clippy::too_many_arguments)]
fn hoist_lambda(
    span: SourceSpan,
    generics: &[GenericType],
    params: &[HirParam],
    return_type: &ValkyrieType,
    body: &mut HirBlock,
    captures: &mut Vec<HirCapture>,
    hoisted: &mut Option<HirLambdaIdentity>,
    context: &mut HoistContext,
    locals: &BTreeMap<String, LocalBinding>,
) {
    let mut analyzer = CaptureAnalyzer::new();
    for (name, binding) in locals {
        analyzer.add_var(name, binding.ty.clone(), binding.is_mutable);
    }

    let mut shadowed = BTreeSet::new();
    for param in params {
        shadowed.insert(param.name.name.to_string());
    }
    collect_block(body, &mut analyzer, &mut shadowed);
    *captures = analyzer.into_captures();

    let id = LAMBDA_COUNTER.fetch_add(1, Ordering::Relaxed);
    let function_name = Identifier::new(&format!("__lambda_{id}"));
    // 合成函数继承 owner 泛型：lambda 体内的 `T` 必须由声明绑定，否则每个
    // owner 实例会共享同一个未闭合的合成签名。环境聚合只继承 owner 泛型，
    // 因为只有 owner 作用域的变量可能被捕获。
    let owner_generics = context.owner_generics.clone();
    let synthetic_generics = merge_generics(&owner_generics, generics);

    let environment = if captures.is_empty() {
        None
    } else {
        let struct_name = Identifier::new(&format!("__lambda_env_{id}"));
        let fields = captures
            .iter()
            .map(|capture| HirField {
                id: None,
                name: environment_field_name(&capture.identifier.name),
                doc: HirDocumentation::default(),
                // 捕获变量的 `SelfType` 必须落成 impl 目标名义类型，环境字段才有
                // 接收者类型，lambda 体内的成员调用才能在 HIR 解析期成立。
                ty: match (&capture.ty, &context.owner_self_type) {
                    (ValkyrieType::SelfType, Some(owner_self_type)) => owner_self_type.clone(),
                    (ty, _) => ty.clone(),
                },
                visibility: HirVisibility::private(),
                is_mutable: capture.is_write,
            })
            .collect::<Vec<_>>();
        context.pending_structs.push(HirStruct {
            name: struct_name.clone(),
            namespace: context.namespace.clone(),
            generics: owner_generics.clone(),
            fields,
            visibility: HirVisibility::private(),
            ..HirStruct::default()
        });
        Some(HirLambdaEnvironment { struct_name, ty: None, field_ids: Vec::new() })
    };

    let mut synthetic_params = params.to_vec();
    if let Some(environment) = &environment {
        // 泛型 owner 的环境形参必须按 `Apply(Named(env), [owner 泛型…])` 书写：
        // 绑定期会把它冻结成带实参的 `Nominal`，而不是缺实参的裸声明。
        let environment_ty = if owner_generics.is_empty() {
            ValkyrieType::Named(environment.struct_name.clone())
        } else {
            ValkyrieType::Apply(
                Box::new(ValkyrieType::Named(environment.struct_name.clone())),
                owner_generics.iter().map(|generic| ValkyrieType::Named(generic.name.clone())).collect(),
            )
        };
        synthetic_params.push(HirParam {
            name: HirIdentifier { name: Identifier::new(ENV_PARAMETER), shadow_index: 0, span: span.clone() },
            ty: environment_ty,
            ..HirParam::default()
        });
    }

    context.pending_functions.push(HirFunction {
        declaration: None,
        instance: None,
        name: function_name.clone(),
        declaring_namespace: NamePath::new(context.namespace.clone()),
        doc: HirDocumentation::default(),
        annotations: Vec::new(),
        generics: synthetic_generics.clone(),
        where_constraints: context.owner_where_constraints.clone(),
        params: synthetic_params,
        return_type: return_type.clone(),
        body: HirBlock { statements: Vec::new(), expr: None, span: body.span.clone() },
        span,
        visibility: HirVisibility::private(),
        is_abstract: false,
        is_final: true,
        is_virtual: false,
        is_override: false,
    });

    *hoisted = Some(HirLambdaIdentity { function_name, declaration: None, instance: None, environment });

    // 嵌套 lambda 的可见外层集合包含当前 lambda 的参数与体内局部，泛型作用域
    // 也扩展为「owner + 当前 lambda」；返回后恢复外层 owner 泛型。
    let previous_owner_generics = std::mem::replace(&mut context.owner_generics, synthetic_generics);
    let mut nested_locals = locals.clone();
    for param in params {
        nested_locals.insert(param.name.name.to_string(), LocalBinding { ty: param.ty.clone(), is_mutable: param.is_mutable });
    }
    hoist_block(body, context, &mut nested_locals);
    context.owner_generics = previous_owner_generics;
}

/// 注册后回填 lambda 的合成函数实例、环境类型与字段身份，并把捕获名改写为
/// 环境形参上的 `FieldAccess` / `StoreField`。MIR 不再做捕获名到局的部判定。
pub fn bind_lambda_environments(module: &mut HirModule) {
    for submodule in &mut module.submodules {
        bind_lambda_environments(submodule);
    }

    let snapshot = BindSnapshot::new(module);
    let mut updates = Vec::new();

    for function in &mut module.functions {
        let mut shadowed = BTreeSet::new();
        bind_block(&mut function.body, &snapshot, &mut updates, None, &mut shadowed);
    }
    for structure in &mut module.structs {
        for method in &mut structure.methods {
            let mut shadowed = BTreeSet::new();
            bind_block(&mut method.body, &snapshot, &mut updates, None, &mut shadowed);
        }
    }
    for implementation in &mut module.impls {
        for method in &mut implementation.methods {
            let mut shadowed = BTreeSet::new();
            bind_block(&mut method.body, &snapshot, &mut updates, None, &mut shadowed);
        }
    }
    for trait_declaration in &mut module.traits {
        for method in trait_declaration.methods.iter_mut().chain(trait_declaration.default_methods.iter_mut()) {
            let mut shadowed = BTreeSet::new();
            bind_block(&mut method.body, &snapshot, &mut updates, None, &mut shadowed);
        }
    }
    for singleton in &mut module.singletons {
        for method in &mut singleton.methods {
            let mut shadowed = BTreeSet::new();
            bind_block(&mut method.body, &snapshot, &mut updates, None, &mut shadowed);
        }
        if let Some(constructor) = &mut singleton.constructor {
            let mut shadowed = BTreeSet::new();
            bind_block(&mut constructor.body, &snapshot, &mut updates, None, &mut shadowed);
        }
        if let Some(finalizer) = &mut singleton.finalizer {
            let mut shadowed = BTreeSet::new();
            bind_block(&mut finalizer.body, &snapshot, &mut updates, None, &mut shadowed);
        }
    }
    for widget in &mut module.widgets {
        for method in &mut widget.methods {
            let mut shadowed = BTreeSet::new();
            bind_block(&mut method.body, &snapshot, &mut updates, None, &mut shadowed);
        }
    }
    for type_function in &mut module.type_functions {
        let mut shadowed = BTreeSet::new();
        bind_block(&mut type_function.body, &snapshot, &mut updates, None, &mut shadowed);
    }
    for statement in &mut module.statements {
        let mut shadowed = BTreeSet::new();
        bind_statement(statement, &snapshot, &mut updates, None, &mut shadowed);
    }

    for (function_name, body) in updates {
        if let Some(function) = module.functions.iter_mut().find(|function| function.name == function_name) {
            function.body = body;
        }
    }
}

struct BindSnapshot {
    functions: BTreeMap<String, (Option<nyar_types::ItemId>, Option<nyar_types::ItemInstanceId>)>,
    environments: BTreeMap<String, (Option<nyar_types::ItemId>, Vec<GenericType>, BTreeMap<String, nyar_types::FieldId>)>,
}

impl BindSnapshot {
    fn new(module: &HirModule) -> Self {
        let functions = module
            .functions
            .iter()
            .map(|function| (function.name.to_string(), (function.declaration, function.instance)))
            .collect();
        let environments = module
            .structs
            .iter()
            .map(|structure| {
                let fields = structure
                    .fields
                    .iter()
                    .filter_map(|field| field.id.map(|id| (field.name.to_string(), id)))
                    .collect();
                (structure.name.to_string(), (structure.constructor_declaration, structure.generics.clone(), fields))
            })
            .collect();
        Self { functions, environments }
    }
}

#[derive(Clone)]
struct CaptureSubstitution {
    environment: HirIdentifier,
    fields: BTreeMap<String, (Identifier, nyar_types::FieldId)>,
}

fn bind_lambda(
    hoisted: &mut Option<HirLambdaIdentity>,
    captures: &[HirCapture],
    body: &mut HirBlock,
    snapshot: &BindSnapshot,
    updates: &mut Vec<(Identifier, HirBlock)>,
    shadowed: &BTreeSet<String>,
) {
    let Some(identity) = hoisted else { return };
    if let Some((declaration, instance)) = snapshot.functions.get(identity.function_name.as_str()) {
        identity.declaration = *declaration;
        identity.instance = *instance;
    }

    let substitution = if let Some(environment) = &mut identity.environment {
        if let Some((declaration, generics, fields)) = snapshot.environments.get(environment.struct_name.as_str()) {
            if let Some(declaration) = *declaration {
                // 实参先写成 `Named` 占位：随后 generic binder 传播会用**外层 callable**
                // 的泛型把它们冻结成同 binder 的 `Generic`，单态化才能按实例代入。
                let arguments = generics.iter().map(|generic| ValkyrieType::Named(generic.name.clone())).collect();
                environment.ty = Some(ValkyrieType::Nominal { declaration, arguments, primitive_identity: None });
            }
            environment.field_ids = captures
                .iter()
                .filter_map(|capture| fields.get(environment_field_name(&capture.identifier.name).as_str()).copied())
                .collect();
        }
        let fields = captures
            .iter()
            .zip(environment.field_ids.iter())
            .map(|(capture, field_id)| (capture.identifier.name.to_string(), (environment_field_name(&capture.identifier.name), *field_id)))
            .collect();
        Some(CaptureSubstitution {
            environment: HirIdentifier { name: Identifier::new(ENV_PARAMETER), shadow_index: 0, span: body.span.clone() },
            fields,
        })
    } else {
        None
    };

    let mut body_shadowed = shadowed.clone();
    // 只在合成函数的副本上改写捕获名：lambda 表达式的原体仍嵌在外层函数里，
    // 语义解析会用外层词法环境遍历它，改写成 `__nyar_env` 会让接收者脱离
    // 该环境而无法求解。原体保持捕获名，合成函数拿到独立改写后的副本。
    let mut bound_body = body.clone();
    bind_block(&mut bound_body, snapshot, updates, substitution.as_ref(), &mut body_shadowed);
    updates.push((identity.function_name.clone(), bound_body));
}

#[allow(clippy::too_many_arguments)]
fn bind_block(
    block: &mut HirBlock,
    snapshot: &BindSnapshot,
    updates: &mut Vec<(Identifier, HirBlock)>,
    substitution: Option<&CaptureSubstitution>,
    shadowed: &mut BTreeSet<String>,
) {
    for statement in &mut block.statements {
        bind_statement(statement, snapshot, updates, substitution, shadowed);
    }
    if let Some(expr) = &mut block.expr {
        bind_expr(expr, snapshot, updates, substitution, shadowed);
    }
}

fn bind_statement(
    statement: &mut HirStatement,
    snapshot: &BindSnapshot,
    updates: &mut Vec<(Identifier, HirBlock)>,
    substitution: Option<&CaptureSubstitution>,
    shadowed: &mut BTreeSet<String>,
) {
    match &mut statement.kind {
        HirStatementKind::Let { pattern, initializer, .. } => {
            if let Some(initializer) = initializer {
                bind_expr(initializer, snapshot, updates, substitution, shadowed);
            }
            let mut names = BTreeSet::new();
            pattern_names(pattern, &mut names);
            shadowed.extend(names);
        }
        HirStatementKind::Expr(expr) => bind_expr(expr, snapshot, updates, substitution, shadowed),
    }
}

fn bind_expr(
    expr: &mut HirExpr,
    snapshot: &BindSnapshot,
    updates: &mut Vec<(Identifier, HirBlock)>,
    substitution: Option<&CaptureSubstitution>,
    shadowed: &mut BTreeSet<String>,
) {
    match &mut expr.kind {
        HirExprKind::Variable(identifier) => {
            let name = identifier.name.to_string();
            if shadowed.contains(name.as_str()) {
                return;
            }
            let Some(substitution) = substitution else { return };
            let Some((field, field_id)) = substitution.fields.get(name.as_str()) else { return };
            let environment = substitution.environment.clone();
            let field = field.clone();
            let field_id = *field_id;
            let span = expr.span.clone();
            expr.kind = HirExprKind::FieldAccess {
                object: Box::new(HirExpr { kind: HirExprKind::Variable(environment), span }),
                field,
                field_id: Some(field_id),
                tuple_index: None,
            };
        }
        // parser 会把成员调用写成 `binding.member` 的多段路径而不是 `FieldAccess`。
        // 捕获名出现在首段时同样必须改写为环境字段，否则合成函数里 `binding` 不在
        // 局部环境，接收者类型无从求解，调用被判为无候选。
        HirExprKind::Path(path) => {
            let Some(head) = path.parts().first().cloned() else { return };
            let head_name = head.to_string();
            if shadowed.contains(head_name.as_str()) {
                return;
            }
            let Some(substitution) = substitution else { return };
            let Some((field, field_id)) = substitution.fields.get(head_name.as_str()) else { return };
            let environment = substitution.environment.clone();
            let field = field.clone();
            let field_id = *field_id;
            let span = expr.span.clone();
            let parts = path.parts().to_vec();
            let mut receiver = HirExpr {
                kind: HirExprKind::FieldAccess {
                    object: Box::new(HirExpr { kind: HirExprKind::Variable(environment), span: span.clone() }),
                    field,
                    field_id: Some(field_id),
                    tuple_index: None,
                },
                span: span.clone(),
            };
            if parts.len() == 1 {
                expr.kind = receiver.kind;
                return;
            }
            for segment in &parts[1..parts.len() - 1] {
                receiver = HirExpr {
                    kind: HirExprKind::FieldAccess {
                        object: Box::new(receiver),
                        field: segment.clone(),
                        field_id: None,
                        tuple_index: None,
                    },
                    span: span.clone(),
                };
            }
            expr.kind = HirExprKind::FieldAccess {
                object: Box::new(receiver),
                field: parts[parts.len() - 1].clone(),
                field_id: None,
                tuple_index: None,
            };
        }
        HirExprKind::Assign { target, value } => {
            bind_expr(value, snapshot, updates, substitution, shadowed);
            let name = target.to_string();
            if shadowed.contains(name.as_str()) {
                return;
            }
            let Some(substitution) = substitution else { return };
            let Some((field, field_id)) = substitution.fields.get(name.as_str()) else { return };
            let environment = substitution.environment.clone();
            let field = field.clone();
            let field_id = *field_id;
            let span = expr.span.clone();
            let stored_value = std::mem::replace(
                value,
                Box::new(HirExpr { kind: HirExprKind::Literal(crate::compiler::types::hir::HirLiteral::Unit), span: span.clone() }),
            );
            expr.kind = HirExprKind::StoreField {
                object: Box::new(HirExpr { kind: HirExprKind::Variable(environment), span }),
                field,
                value: stored_value,
                field_id: Some(field_id),
            };
        }
        HirExprKind::Lambda { params, body, captures, hoisted, .. } => {
            for param in params.iter_mut() {
                if let Some(default) = &mut param.default {
                    bind_expr(default, snapshot, updates, substitution, shadowed);
                }
            }
            // 合成函数是独立项：外层函数的局部不在其作用域内，捕获名必须被改写。
            // 只有 lambda 自身形参与体内 `let` 才构成合法遮蔽。
            let mut lambda_shadowed = BTreeSet::new();
            for param in params.iter() {
                lambda_shadowed.insert(param.name.name.to_string());
            }
            bind_lambda(hoisted, captures, body, snapshot, updates, &lambda_shadowed);
        }
        HirExprKind::AnonymousClass { fields, .. } => {
            for (_, value) in fields {
                bind_expr(value, snapshot, updates, substitution, shadowed);
            }
        }
        HirExprKind::Call { callee, args, .. } => {
            bind_expr(callee, snapshot, updates, substitution, shadowed);
            for arg in args {
                bind_expr(&mut arg.value, snapshot, updates, substitution, shadowed);
            }
        }
        HirExprKind::Construct { args, .. } => {
            for arg in args {
                bind_expr(arg, snapshot, updates, substitution, shadowed);
            }
        }
        HirExprKind::FieldInit { value, .. }
        | HirExprKind::ArrayNew { length: value, .. }
        | HirExprKind::FieldAccess { object: value, .. }
        | HirExprKind::GenericApply { callee: value, .. }
        | HirExprKind::Return(Some(value))
        | HirExprKind::Yield(Some(value))
        | HirExprKind::Resume(value)
        | HirExprKind::Raise(value)
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::Cast { expr: value, .. } => bind_expr(value, snapshot, updates, substitution, shadowed),
        HirExprKind::TryPropagate { inner: value, .. } => bind_expr(value, snapshot, updates, substitution, shadowed),
        HirExprKind::StoreField { object, value, .. } => {
            bind_expr(object, snapshot, updates, substitution, shadowed);
            bind_expr(value, snapshot, updates, substitution, shadowed);
        }
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } | HirExprKind::CollectionInit { items, .. } => {
            for item in items {
                bind_expr(item, snapshot, updates, substitution, shadowed);
            }
        }
        HirExprKind::Block(block) => {
            let mut block_shadowed = shadowed.clone();
            bind_block(block, snapshot, updates, substitution, &mut block_shadowed);
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            bind_expr(condition, snapshot, updates, substitution, shadowed);
            let mut then_shadowed = shadowed.clone();
            bind_block(then_branch, snapshot, updates, substitution, &mut then_shadowed);
            if let Some(else_branch) = else_branch {
                let mut else_shadowed = shadowed.clone();
                bind_block(else_branch, snapshot, updates, substitution, &mut else_shadowed);
            }
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            bind_expr(scrutinee, snapshot, updates, substitution, shadowed);
            let mut then_shadowed = shadowed.clone();
            let mut names = BTreeSet::new();
            pattern_names(pattern, &mut names);
            then_shadowed.extend(names);
            bind_block(then_branch, snapshot, updates, substitution, &mut then_shadowed);
            if let Some(else_branch) = else_branch {
                let mut else_shadowed = shadowed.clone();
                bind_block(else_branch, snapshot, updates, substitution, &mut else_shadowed);
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            bind_expr(scrutinee, snapshot, updates, substitution, shadowed);
            for arm in arms {
                bind_match_arm(arm, snapshot, updates, substitution, shadowed);
            }
        }
        HirExprKind::Loop { pattern, iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                bind_expr(iterator, snapshot, updates, substitution, shadowed);
            }
            if let Some(condition) = condition {
                bind_expr(condition, snapshot, updates, substitution, shadowed);
            }
            let mut body_shadowed = shadowed.clone();
            if let Some(pattern) = pattern {
                let mut names = BTreeSet::new();
                pattern_names(pattern, &mut names);
                body_shadowed.extend(names);
            }
            bind_block(body, snapshot, updates, substitution, &mut body_shadowed);
        }
        HirExprKind::Break { expr: Some(value), .. } => bind_expr(value, snapshot, updates, substitution, shadowed),
        HirExprKind::Catch { expr: value, arms } => {
            bind_expr(value, snapshot, updates, substitution, shadowed);
            for arm in arms {
                bind_match_arm(arm, snapshot, updates, substitution, shadowed);
            }
        }
        HirExprKind::TryScope { body, .. } => {
            let mut body_shadowed = shadowed.clone();
            bind_block(body, snapshot, updates, substitution, &mut body_shadowed);
        }
        HirExprKind::With { base, updates: field_updates } => {
            bind_expr(base, snapshot, updates, substitution, shadowed);
            for (_, value) in field_updates {
                bind_expr(value, snapshot, updates, substitution, shadowed);
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                bind_expr(arg, snapshot, updates, substitution, shadowed);
            }
        }
        _ => {}
    }
}

fn bind_match_arm(
    arm: &mut HirMatchArm,
    snapshot: &BindSnapshot,
    updates: &mut Vec<(Identifier, HirBlock)>,
    substitution: Option<&CaptureSubstitution>,
    shadowed: &BTreeSet<String>,
) {
    let mut arm_shadowed = shadowed.clone();
    let mut names = BTreeSet::new();
    pattern_names(&arm.pattern, &mut names);
    arm_shadowed.extend(names);
    if let Some(guard) = &mut arm.guard {
        bind_expr(guard, snapshot, updates, substitution, &mut arm_shadowed);
    }
    bind_expr(&mut arm.body, snapshot, updates, substitution, &mut arm_shadowed);
}

fn collect_block(block: &HirBlock, analyzer: &mut CaptureAnalyzer, shadowed: &mut BTreeSet<String>) {
    for statement in &block.statements {
        collect_statement(statement, analyzer, shadowed);
    }
    if let Some(expr) = &block.expr {
        collect_expr(expr, analyzer, shadowed);
    }
}

fn collect_statement(statement: &HirStatement, analyzer: &mut CaptureAnalyzer, shadowed: &mut BTreeSet<String>) {
    match &statement.kind {
        HirStatementKind::Let { pattern, initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_expr(initializer, analyzer, shadowed);
            }
            let mut names = BTreeSet::new();
            pattern_names(pattern, &mut names);
            shadowed.extend(names);
        }
        HirStatementKind::Expr(expr) => collect_expr(expr, analyzer, shadowed),
    }
}

fn collect_expr(expr: &HirExpr, analyzer: &mut CaptureAnalyzer, shadowed: &BTreeSet<String>) {
    match &expr.kind {
        HirExprKind::Variable(identifier) => {
            if !shadowed.contains(identifier.name.as_str()) {
                analyzer.access_var(identifier.name.as_str(), false);
            }
        }
        // parser 会把裸局部引用保留成单段 Path，把成员调用保留成多段 Path；
        // 首段才是接收者绑定，末段是成员名。漏掉这两种形状会导致闭包不登记捕获，
        // 合成函数里接收者无类型可解。
        HirExprKind::Path(path) => {
            let Some(head) = path.parts().first() else { return };
            if !shadowed.contains(head.as_str()) {
                analyzer.access_var(head.as_str(), false);
            }
        }
        HirExprKind::Assign { target, value } => {
            if !shadowed.contains(target.as_str()) {
                analyzer.access_var(target.as_str(), true);
            }
            collect_expr(value, analyzer, shadowed);
        }
        // 嵌套 lambda / 匿名 class 各自处理自己的捕获集合。
        HirExprKind::Lambda { .. } | HirExprKind::AnonymousClass { .. } => {}
        HirExprKind::Call { callee, args, .. } => {
            collect_expr(callee, analyzer, shadowed);
            for arg in args {
                collect_expr(&arg.value, analyzer, shadowed);
            }
        }
        HirExprKind::Construct { args, .. } => {
            for arg in args {
                collect_expr(arg, analyzer, shadowed);
            }
        }
        HirExprKind::FieldInit { value, .. }
        | HirExprKind::ArrayNew { length: value, .. }
        | HirExprKind::FieldAccess { object: value, .. }
        | HirExprKind::GenericApply { callee: value, .. }
        | HirExprKind::Return(Some(value))
        | HirExprKind::Yield(Some(value))
        | HirExprKind::Resume(value)
        | HirExprKind::TryPropagate { inner: value, .. }
        | HirExprKind::Raise(value)
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::Cast { expr: value, .. } => collect_expr(value, analyzer, shadowed),
        HirExprKind::StoreField { object, value, .. } => {
            collect_expr(object, analyzer, shadowed);
            collect_expr(value, analyzer, shadowed);
        }
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } | HirExprKind::CollectionInit { items, .. } => {
            for item in items {
                collect_expr(item, analyzer, shadowed);
            }
        }
        HirExprKind::Block(block) => {
            let mut block_shadowed = shadowed.clone();
            collect_block(block, analyzer, &mut block_shadowed);
        }
        HirExprKind::If { condition, then_branch, else_branch } => {
            collect_expr(condition, analyzer, shadowed);
            let mut then_shadowed = shadowed.clone();
            collect_block(then_branch, analyzer, &mut then_shadowed);
            if let Some(else_branch) = else_branch {
                let mut else_shadowed = shadowed.clone();
                collect_block(else_branch, analyzer, &mut else_shadowed);
            }
        }
        HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
            collect_expr(scrutinee, analyzer, shadowed);
            let mut then_shadowed = shadowed.clone();
            let mut names = BTreeSet::new();
            pattern_names(pattern, &mut names);
            then_shadowed.extend(names);
            collect_block(then_branch, analyzer, &mut then_shadowed);
            if let Some(else_branch) = else_branch {
                let mut else_shadowed = shadowed.clone();
                collect_block(else_branch, analyzer, &mut else_shadowed);
            }
        }
        HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
            collect_expr(scrutinee, analyzer, shadowed);
            for arm in arms {
                collect_match_arm(arm, analyzer, shadowed);
            }
        }
        HirExprKind::Loop { pattern, iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                collect_expr(iterator, analyzer, shadowed);
            }
            if let Some(condition) = condition {
                collect_expr(condition, analyzer, shadowed);
            }
            let mut body_shadowed = shadowed.clone();
            if let Some(pattern) = pattern {
                let mut names = BTreeSet::new();
                pattern_names(pattern, &mut names);
                body_shadowed.extend(names);
            }
            collect_block(body, analyzer, &mut body_shadowed);
        }
        HirExprKind::Break { expr: Some(value), .. } => collect_expr(value, analyzer, shadowed),
        HirExprKind::Catch { expr: value, arms } => {
            collect_expr(value, analyzer, shadowed);
            for arm in arms {
                collect_match_arm(arm, analyzer, shadowed);
            }
        }
        HirExprKind::TryScope { body, .. } => {
            let mut body_shadowed = shadowed.clone();
            collect_block(body, analyzer, &mut body_shadowed);
        }
        HirExprKind::With { base, updates } => {
            collect_expr(base, analyzer, shadowed);
            for (_, value) in updates {
                collect_expr(value, analyzer, shadowed);
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                collect_expr(arg, analyzer, shadowed);
            }
        }
        _ => {}
    }
}

fn collect_match_arm(arm: &HirMatchArm, analyzer: &mut CaptureAnalyzer, shadowed: &BTreeSet<String>) {
    let mut arm_shadowed = shadowed.clone();
    let mut names = BTreeSet::new();
    pattern_names(&arm.pattern, &mut names);
    arm_shadowed.extend(names);
    if let Some(guard) = &arm.guard {
        collect_expr(guard, analyzer, &arm_shadowed);
    }
    collect_expr(&arm.body, analyzer, &arm_shadowed);
}

fn pattern_names(pattern: &HirPattern, names: &mut BTreeSet<String>) {
    match pattern {
        HirPattern::Variable(identifier) | HirPattern::TypedBind { identifier, .. } => {
            names.insert(identifier.name.to_string());
        }
        HirPattern::Bind { identifier, pattern } => {
            names.insert(identifier.name.to_string());
            pattern_names(pattern, names);
        }
        HirPattern::Tuple(patterns) | HirPattern::Or(patterns) => {
            for pattern in patterns {
                pattern_names(pattern, names);
            }
        }
        HirPattern::Extractor(extractor) => match extractor {
            HirExtractorPattern::Constructor { fields, .. } => {
                for pattern in fields {
                    pattern_names(pattern, names);
                }
            }
            HirExtractorPattern::Array { prefix, rest, suffix, .. } => {
                for pattern in prefix.iter().chain(suffix.iter()) {
                    pattern_names(pattern, names);
                }
                if let Some(rest) = rest {
                    names.insert(rest.name.to_string());
                }
            }
        },
        HirPattern::Object { fields, rest, .. } => {
            for (_, pattern) in fields {
                pattern_names(pattern, names);
            }
            if let Some(rest) = rest {
                names.insert(rest.name.to_string());
            }
        }
        HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => pattern_names(pattern, names),
        _ => {}
    }
}

fn bind_pattern_names(pattern: &HirPattern, ty: &ValkyrieType, is_mutable: bool, locals: &mut BTreeMap<String, LocalBinding>) {
    let mut names = BTreeSet::new();
    pattern_names(pattern, &mut names);
    for name in names {
        locals.insert(name, LocalBinding { ty: ty.clone(), is_mutable });
    }
}

fn environment_field_name(name: &Identifier) -> Identifier {
    Identifier::new(&format!("__cap_{}", name.as_str()))
}

fn infer_expr_type(expr: &HirExpr, locals: &BTreeMap<String, LocalBinding>) -> Option<ValkyrieType> {
    match &expr.kind {
        HirExprKind::Literal(literal) => Some(match literal {
            crate::compiler::types::hir::HirLiteral::Integer64(_) => ValkyrieType::Integer64 { signed: true },
            crate::compiler::types::hir::HirLiteral::Float64(_) => ValkyrieType::Float64,
            crate::compiler::types::hir::HirLiteral::Bool(_) => ValkyrieType::Boolean,
            crate::compiler::types::hir::HirLiteral::Unit => ValkyrieType::Unit,
            crate::compiler::types::hir::HirLiteral::Character(_) => ValkyrieType::Character,
            crate::compiler::types::hir::HirLiteral::Byte(_) => ValkyrieType::Integer8 { signed: false },
            crate::compiler::types::hir::HirLiteral::String(_) => ValkyrieType::Named(Identifier::new("utf8")),
            crate::compiler::types::hir::HirLiteral::Null => return None,
        }),
        HirExprKind::Variable(identifier) => locals.get(identifier.name.as_str()).map(|binding| binding.ty.clone()),
        _ => None,
    }
}
