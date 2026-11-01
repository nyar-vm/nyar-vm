//! 物化可达性门：semantic-closure-v1 闭包只是名字解析上界，本模块在 HIR 冻结调用图上
//! 从根集合求「真正需要物化」的 callable 可达闭包。
//!
//! 根集合：`[export]` / `[main]` / `[test]` 表面注解成员、Resolver 选定 `[host_provider]`
//! 的 callable 族、导出面具体聚合的 `[library_glue]` 成员，以及 singleton 成员
//! （其唯一实例由 backend init/unload 生命周期路径直通调用，属程序级义务而非调用边）。
//! BFS 沿 `HirExprKind::Call` / `Construct` / `CollectionInit` / `TryPropagate` /
//! `FunctionReference` 的冻结 callee 边（含 witness `trait_dispatch` 双声明身份与
//! `sum_owner` / `sum_variant` evidence）以及签名/体内全部类型引用展开。
//!
//! 不可达 callable 不得进入泛型实例化、MIR 降低与 backend plan / `item_instances`。
//! 门只裁物化集合，不触碰闭包上界与 closure hash；禁止名字表、短名特判或硬编码豁免。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use nyar_types::{ItemId, ItemInstanceId, builtin_attribute};

use crate::compiler::{
    emitter::pipeline::{CompilerHostProviderBinding, host_provider_root_declarations},
    syntax::oak::ParseError,
    types::hir::{
        HirBlock, HirExpr, HirExprKind, HirFunction, HirLiteral, HirMatchArm, HirModule, HirPattern, HirResolvedCall, HirStatement,
        HirStatementKind, HirStringLiteral, HirWhereConstraint, ValkyrieType, export_spec::parse_library_glue_operation,
        parse_export_spec_from_annotations, resolve_attribute_id,
    },
};

/// HIR 冻结调用图上的物化可达闭包。
///
/// `reachable_declarations` 是泛型实例化与 MIR 发射的 callable 声明门禁；
/// `reachable_instances` 在单态化固定点之后由 [`HirMaterializationGate::finalize_instances`]
/// 收口，覆盖铸造实例与 witness 消解后的 impl 实例。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirMaterializationGate {
    reachable_declarations: BTreeSet<ItemId>,
    reachable_instances: BTreeSet<ItemInstanceId>,
}

impl HirMaterializationGate {
    /// callable 声明身份是否允许物化；无声明身份的对象不裁剪。
    pub(crate) fn permits_declaration(&self, declaration: Option<ItemId>) -> bool {
        declaration.is_none_or(|declaration| self.reachable_declarations.contains(&declaration))
    }

    /// callable 实例身份是否允许物化；无实例身份的对象不裁剪。
    pub(crate) fn permits_instance(&self, instance: Option<ItemInstanceId>) -> bool {
        instance.is_none_or(|instance| self.reachable_instances.contains(&instance))
    }

    /// 可达 callable 声明集合（只读事实面，供诊断与回归断言）。
    pub fn reachable_declarations(&self) -> &BTreeSet<ItemId> {
        &self.reachable_declarations
    }

    /// 可达 callable 实例集合（只读事实面，供诊断与回归断言）。
    pub fn reachable_instances(&self) -> &BTreeSet<ItemInstanceId> {
        &self.reachable_instances
    }

    /// 单态化固定点之后收口实例集合。
    ///
    /// 声明级实例来自被允许声明的 callable；铸造实例来自被允许声明 callable 体内
    /// 已回填的 resolved callee（含 witness 消解到 impl 成员的实例——其声明身份不在
    /// 可达集，但实例被可达调用点直接引用）。
    pub(crate) fn finalize_instances(&mut self, modules: &[HirModule]) {
        let mut callables = Vec::new();
        for module in modules {
            collect_module_callables(module, &mut callables);
        }
        for function in &callables {
            if self.permits_declaration(function.declaration)
                && let Some(instance) = function.instance
            {
                self.reachable_instances.insert(instance);
            }
        }
        for module in modules {
            collect_instantiated_instances(module, self);
        }
        let mut walker = ReachabilityWalker::collecting();
        for function in &callables {
            if self.permits_declaration(function.declaration) {
                walker.visit_callable(function);
            }
        }
        self.reachable_instances.extend(walker.instances);
    }
}

/// 在冻结调用图上求物化可达闭包（实例集合待单态化后 `finalize_instances` 收口）。
///
/// 公开为只读事实入口：管线内部在单态化前调用；测试与诊断可对已物化 HIR 复算
/// 声明可达集（单态化只回填实例身份、不改变声明 callee 边，声明集与管线内一致）。
pub fn compute_hir_materialization_gate(
    modules: &[HirModule],
    host_bindings: &[CompilerHostProviderBinding],
) -> Result<HirMaterializationGate, ParseError> {
    let mut callables = Vec::new();
    for module in modules {
        collect_module_callables(module, &mut callables);
    }
    let mut walker = ReachabilityWalker::growing();
    // 根 1：表面注解成员（export / test / main），覆盖全部 callable 位置。
    for function in &callables {
        if is_surface_root(function) {
            walker.mark(function.declaration);
        }
    }
    // 根 2：Resolver 选定 host provider 的 callable 族声明。
    let hirs: Vec<&HirModule> = modules.iter().collect();
    for declaration in host_provider_root_declarations(&hirs, host_bindings)? {
        walker.mark(Some(declaration));
    }
    // 根 3：singleton 成员（生命周期直通义务）。
    for module in modules {
        collect_singleton_roots(module, &mut walker);
    }
    // 根 4：导出面具体聚合的 `[library_glue]` 成员。
    let mut exported_nominals = BTreeSet::new();
    for module in modules {
        collect_exported_nominal_declarations(module, &mut exported_nominals);
    }
    for module in modules {
        collect_glue_roots(module, &exported_nominals, &mut walker);
    }
    walker.index_callables(&callables);
    walker.run();
    Ok(HirMaterializationGate { reachable_declarations: walker.declarations, reachable_instances: BTreeSet::new() })
}

/// 表面注解根：`[export]` / `[test]` / `[main]`。
fn is_surface_root(function: &HirFunction) -> bool {
    if parse_export_spec_from_annotations(&function.annotations).is_some() {
        return true;
    }
    function.annotations.iter().any(
        |attribute| matches!(resolve_attribute_id(attribute), Some(id) if id == builtin_attribute::test() || id == builtin_attribute::main()),
    )
}

/// 收集模块（含子模块）内全部声明级 callable；不含单态化铸造副本。
fn collect_module_callables<'a>(module: &'a HirModule, out: &mut Vec<&'a HirFunction>) {
    out.extend(module.functions.iter());
    for structure in &module.structs {
        out.extend(structure.methods.iter());
        for property in &structure.properties {
            out.extend(property.getter.iter().chain(property.setter.iter()));
        }
    }
    for implementation in &module.impls {
        out.extend(implementation.methods.iter());
    }
    for trait_definition in &module.traits {
        out.extend(trait_definition.methods.iter().chain(trait_definition.default_methods.iter()));
    }
    for singleton in &module.singletons {
        out.extend(singleton.methods.iter());
        out.extend(singleton.constructor.iter().map(Box::as_ref));
        out.extend(singleton.finalizer.iter().map(Box::as_ref));
    }
    for widget in &module.widgets {
        out.extend(widget.methods.iter());
    }
    for submodule in &module.submodules {
        collect_module_callables(submodule, out);
    }
}

/// 铸造副本的实例登记：声明被允许即实例被允许。
fn collect_instantiated_instances(module: &HirModule, gate: &mut HirMaterializationGate) {
    for callable in &module.instantiated_callables {
        if gate.permits_declaration(callable.function.declaration)
            && let Some(instance) = callable.function.instance
        {
            gate.reachable_instances.insert(instance);
        }
    }
    for submodule in &module.submodules {
        collect_instantiated_instances(submodule, gate);
    }
}

/// singleton 成员根：唯一实例由 backend 生命周期路径直通调用。
fn collect_singleton_roots(module: &HirModule, walker: &mut ReachabilityWalker<'_>) {
    for singleton in &module.singletons {
        for method in
            singleton.methods.iter().chain(singleton.constructor.iter().map(Box::as_ref)).chain(singleton.finalizer.iter().map(Box::as_ref))
        {
            walker.mark(method.declaration);
        }
    }
    for submodule in &module.submodules {
        collect_singleton_roots(submodule, walker);
    }
}

/// 导出函数签名中出现的名义声明身份（含嵌套实参）。
fn collect_exported_nominal_declarations(module: &HirModule, out: &mut BTreeSet<ItemId>) {
    for function in &module.functions {
        if parse_export_spec_from_annotations(&function.annotations).is_none() {
            continue;
        }
        for param in &function.params {
            collect_nominal_declarations(&param.ty, out);
        }
        collect_nominal_declarations(&function.return_type, out);
    }
    for submodule in &module.submodules {
        collect_exported_nominal_declarations(submodule, out);
    }
}

/// `[library_glue]` 成员根：owner 聚合出现在导出面即直通放行；owner 不可解析
/// （自由函数等）时保持今日物化事实，不做按名回查。
fn collect_glue_roots(module: &HirModule, exported: &BTreeSet<ItemId>, walker: &mut ReachabilityWalker<'_>) {
    for function in &module.functions {
        if parse_library_glue_operation(&function.annotations).is_some() {
            walker.mark(function.declaration);
        }
    }
    for structure in &module.structs {
        for method in &structure.methods {
            if parse_library_glue_operation(&method.annotations).is_some() && glue_owner_permitted(structure.constructor_declaration, exported)
            {
                walker.mark(method.declaration);
            }
        }
    }
    for implementation in &module.impls {
        let owner = implementation.target.nominal_declaration();
        for method in &implementation.methods {
            if parse_library_glue_operation(&method.annotations).is_some() && glue_owner_permitted(owner, exported) {
                walker.mark(method.declaration);
            }
        }
    }
    for trait_definition in &module.traits {
        for method in &trait_definition.default_methods {
            if parse_library_glue_operation(&method.annotations).is_some() && glue_owner_permitted(trait_definition.declaration, exported) {
                walker.mark(method.declaration);
            }
        }
    }
    for submodule in &module.submodules {
        collect_glue_roots(submodule, exported, walker);
    }
}

fn glue_owner_permitted(owner: Option<ItemId>, exported: &BTreeSet<ItemId>) -> bool {
    match owner {
        Some(owner) => exported.contains(&owner),
        None => true,
    }
}

/// 冻结调用图 BFS 行走器；`collecting` 模式只回收 resolved 实例身份。
struct ReachabilityWalker<'a> {
    /// 声明身份 → 携带该声明的 callable（可能多个重载共享声明）。
    by_declaration: BTreeMap<ItemId, Vec<&'a HirFunction>>,
    pending: VecDeque<ItemId>,
    declarations: BTreeSet<ItemId>,
    instances: BTreeSet<ItemInstanceId>,
    collecting: bool,
}

impl<'a> ReachabilityWalker<'a> {
    fn growing() -> Self {
        Self {
            by_declaration: BTreeMap::new(),
            pending: VecDeque::new(),
            declarations: BTreeSet::new(),
            instances: BTreeSet::new(),
            collecting: false,
        }
    }

    fn collecting() -> Self {
        Self {
            by_declaration: BTreeMap::new(),
            pending: VecDeque::new(),
            declarations: BTreeSet::new(),
            instances: BTreeSet::new(),
            collecting: true,
        }
    }

    /// 以全部声明级 callable 建立 callee 边索引（grow 模式）。
    fn index_callables(&mut self, callables: &[&'a HirFunction]) {
        for function in callables {
            if let Some(declaration) = function.declaration {
                self.by_declaration.entry(declaration).or_default().push(*function);
            }
        }
    }

    fn mark(&mut self, declaration: Option<ItemId>) {
        if self.collecting {
            return;
        }
        let Some(declaration) = declaration
        else {
            return;
        };
        if self.declarations.insert(declaration) {
            self.pending.push_back(declaration);
        }
    }

    fn note_type_declaration(&mut self, declaration: ItemId) {
        if self.collecting {
            return;
        }
        self.declarations.insert(declaration);
    }

    fn note_instance(&mut self, instance: Option<ItemInstanceId>) {
        if self.collecting {
            if let Some(instance) = instance {
                self.instances.insert(instance);
            }
        }
    }

    fn run(&mut self) {
        // 根先入索引再展开：mark 在 index 之前只写集合与队列。
        while let Some(declaration) = self.pending.pop_front() {
            if let Some(callables) = self.by_declaration.get(&declaration) {
                let callables = callables.clone();
                for function in callables {
                    self.visit_callable(function);
                }
            }
        }
    }

    fn visit_callable(&mut self, function: &HirFunction) {
        for param in &function.params {
            self.visit_type(&param.ty);
            if let Some(default) = &param.default {
                self.visit_expr(default);
            }
        }
        self.visit_type(&function.return_type);
        for constraint in &function.where_constraints {
            self.visit_where_constraint(constraint);
        }
        self.visit_block(&function.body);
    }

    fn visit_where_constraint(&mut self, constraint: &HirWhereConstraint) {
        self.visit_type(&constraint.target);
        for bound in &constraint.bounds {
            if let Some(declaration) = bound.declaration {
                self.note_type_declaration(declaration);
            }
            for argument in &bound.type_arguments {
                self.visit_type(argument);
            }
            for associated in &bound.associated_types {
                self.visit_type(&associated.ty);
            }
        }
    }

    fn visit_block(&mut self, block: &HirBlock) {
        for statement in &block.statements {
            self.visit_statement(statement);
        }
        if let Some(expr) = &block.expr {
            self.visit_expr(expr);
        }
    }

    fn visit_statement(&mut self, statement: &HirStatement) {
        match &statement.kind {
            HirStatementKind::Let { pattern, initializer, ty, .. } => {
                self.visit_pattern(pattern);
                if let Some(initializer) = initializer {
                    self.visit_expr(initializer);
                }
                if let Some(ty) = ty {
                    self.visit_type(ty);
                }
            }
            HirStatementKind::Expr(expr) => self.visit_expr(expr),
        }
    }

    fn visit_pattern(&mut self, pattern: &HirPattern) {
        match pattern {
            HirPattern::Wildcard | HirPattern::Variable(_) | HirPattern::Literal(_) | HirPattern::Range { .. } | HirPattern::Name(_) => {}
            HirPattern::Tuple(items) | HirPattern::Or(items) => {
                for item in items {
                    self.visit_pattern(item);
                }
            }
            HirPattern::Extractor(extractor) => match extractor {
                crate::compiler::types::hir::HirExtractorPattern::Constructor { fields, resolved, .. } => {
                    for field in fields {
                        self.visit_pattern(field);
                    }
                    if let Some(resolved) = resolved {
                        self.visit_resolved_call(resolved);
                    }
                }
                crate::compiler::types::hir::HirExtractorPattern::Array { prefix, suffix, resolved, .. } => {
                    for item in prefix.iter().chain(suffix.iter()) {
                        self.visit_pattern(item);
                    }
                    if let Some(resolved) = resolved {
                        self.visit_resolved_call(resolved);
                    }
                }
            },
            HirPattern::Type { declaration, .. } => {
                if let Some(declaration) = declaration {
                    self.note_type_declaration(*declaration);
                }
            }
            HirPattern::Object { type_declaration, fields, .. } => {
                if let Some(declaration) = type_declaration {
                    self.note_type_declaration(*declaration);
                }
                for (_, subpattern) in fields {
                    self.visit_pattern(subpattern);
                }
            }
            HirPattern::Else => {}
            HirPattern::Bind { pattern, .. } | HirPattern::Mut(pattern) => self.visit_pattern(pattern),
            HirPattern::Pin { pattern, .. } => self.visit_pattern(pattern),
            HirPattern::TypedBind { .. } => {}
        }
    }

    fn visit_match_arm(&mut self, arm: &HirMatchArm) {
        self.visit_pattern(&arm.pattern);
        if let Some(guard) = &arm.guard {
            self.visit_expr(guard);
        }
        self.visit_expr(&arm.body);
    }

    fn visit_resolved_call(&mut self, call: &HirResolvedCall) {
        self.mark(call.declaration);
        self.note_instance(call.instance);
        if let Some(dispatch) = &call.trait_dispatch {
            self.mark(Some(dispatch.trait_declaration));
            self.mark(Some(dispatch.method_declaration));
            self.visit_type(&dispatch.self_type);
        }
        if let Some(owner) = call.sum_owner {
            self.note_type_declaration(owner);
        }
        if let Some(variant) = call.sum_variant {
            self.note_type_declaration(variant);
        }
        self.visit_type(&call.return_type);
        for parameter in &call.parameter_types {
            self.visit_type(parameter);
        }
        if let Some(payload) = &call.extractor_payload_type {
            self.visit_type(payload);
        }
    }

    fn visit_expr(&mut self, expr: &HirExpr) {
        match &expr.kind {
            HirExprKind::Literal(literal) => self.visit_literal(literal),
            HirExprKind::Variable(_) | HirExprKind::Path(_) | HirExprKind::Fallthrough => {}
            HirExprKind::Call { callee, args, resolved } => {
                self.visit_expr(callee);
                for arg in args {
                    self.visit_expr(&arg.value);
                }
                if let Some(resolved) = resolved {
                    self.visit_resolved_call(resolved);
                }
            }
            HirExprKind::Construct { args, resolved, .. } => {
                for arg in args {
                    self.visit_expr(arg);
                }
                if let Some(resolved) = resolved {
                    self.visit_resolved_call(resolved);
                }
            }
            HirExprKind::FieldInit { value, .. } => self.visit_expr(value),
            HirExprKind::ArrayNew { element_type, length } => {
                self.visit_type(element_type);
                self.visit_expr(length);
            }
            HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } => {
                for item in items {
                    self.visit_expr(item);
                }
            }
            HirExprKind::CollectionInit { container_type, element_type, with_length, append, items } => {
                self.visit_type(container_type);
                self.visit_type(element_type);
                self.visit_resolved_call(with_length);
                self.visit_resolved_call(append);
                for item in items {
                    self.visit_expr(item);
                }
            }
            HirExprKind::FieldAccess { object, .. } => self.visit_expr(object),
            HirExprKind::StoreField { object, value, .. } => {
                self.visit_expr(object);
                self.visit_expr(value);
            }
            HirExprKind::GenericApply { callee, arguments } => {
                self.visit_expr(callee);
                for argument in arguments {
                    self.visit_type(argument);
                }
            }
            HirExprKind::Block(block) => self.visit_block(block),
            HirExprKind::Lambda { params, return_type, body, .. } => {
                for param in params {
                    self.visit_type(&param.ty);
                    if let Some(default) = &param.default {
                        self.visit_expr(default);
                    }
                }
                self.visit_type(return_type);
                self.visit_block(body);
            }
            HirExprKind::AnonymousClass { parents, fields, methods, captures, .. } => {
                for parent in parents {
                    for generic in &parent.generics {
                        self.visit_type(generic);
                    }
                }
                for (_, value) in fields {
                    self.visit_expr(value);
                }
                for method in methods {
                    self.visit_callable(method);
                }
                for capture in captures {
                    self.visit_type(&capture.ty);
                }
            }
            HirExprKind::If { condition, then_branch, else_branch } => {
                self.visit_expr(condition);
                self.visit_block(then_branch);
                if let Some(branch) = else_branch {
                    self.visit_block(branch);
                }
            }
            HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
                self.visit_pattern(pattern);
                self.visit_expr(scrutinee);
                self.visit_block(then_branch);
                if let Some(branch) = else_branch {
                    self.visit_block(branch);
                }
            }
            HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
                self.visit_expr(scrutinee);
                for arm in arms {
                    self.visit_match_arm(arm);
                }
            }
            HirExprKind::Loop { pattern, iterator, condition, body, .. } => {
                if let Some(pattern) = pattern {
                    self.visit_pattern(pattern);
                }
                if let Some(iterator) = iterator {
                    self.visit_expr(iterator);
                }
                if let Some(condition) = condition {
                    self.visit_expr(condition);
                }
                self.visit_block(body);
            }
            HirExprKind::Return(expr) | HirExprKind::Yield(expr) => {
                if let Some(expr) = expr {
                    self.visit_expr(expr);
                }
            }
            HirExprKind::Assign { value, .. } => self.visit_expr(value),
            HirExprKind::Break { expr, .. } => {
                if let Some(expr) = expr {
                    self.visit_expr(expr);
                }
            }
            HirExprKind::Continue { .. } => {}
            HirExprKind::Await(expr)
            | HirExprKind::Awake(expr)
            | HirExprKind::BlockOn(expr)
            | HirExprKind::Raise(expr)
            | HirExprKind::YieldFrom(expr)
            | HirExprKind::Resume(expr) => self.visit_expr(expr),
            HirExprKind::Catch { expr, arms } => {
                self.visit_expr(expr);
                for arm in arms {
                    self.visit_match_arm(arm);
                }
            }
            HirExprKind::TryPropagate { inner, resolved_ok_variant } => {
                self.visit_expr(inner);
                if let Some(variant) = resolved_ok_variant {
                    self.visit_resolved_call(variant);
                }
            }
            HirExprKind::TryScope { result_type, body, .. } => {
                if let Some(result_type) = result_type {
                    self.visit_type(result_type);
                }
                self.visit_block(body);
            }
            HirExprKind::Cast { expr, ty } => {
                self.visit_expr(expr);
                self.visit_type(ty);
            }
            HirExprKind::With { base, updates } => {
                self.visit_expr(base);
                for (_, update) in updates {
                    self.visit_expr(update);
                }
            }
            HirExprKind::SuperCall { args, .. } => {
                for arg in args {
                    self.visit_expr(arg);
                }
            }
            HirExprKind::FunctionReference(reference) => {
                self.mark(reference.declaration);
                self.note_instance(reference.instance);
                for parameter in &reference.signature.params {
                    self.visit_type(parameter);
                }
                self.visit_type(&reference.signature.return_type);
            }
        }
    }

    fn visit_literal(&mut self, literal: &HirLiteral) {
        if let HirLiteral::String(string) = literal {
            self.visit_string_literal(string);
        }
    }

    fn visit_string_literal(&mut self, string: &HirStringLiteral) {
        for segment in &string.segments {
            if let crate::compiler::types::hir::HirStringSegment::Interpolation { expr, .. } = segment {
                self.visit_expr(expr);
            }
        }
    }

    fn visit_type(&mut self, ty: &ValkyrieType) {
        if self.collecting {
            return;
        }
        collect_nominal_declarations(ty, &mut self.declarations);
    }
}

/// 递归收集类型里全部名义声明身份（类型事实，只入集合不入队列）。
fn collect_nominal_declarations(ty: &ValkyrieType, out: &mut BTreeSet<ItemId>) {
    if let Some(declaration) = ty.nominal_declaration() {
        out.insert(declaration);
    }
    match ty {
        ValkyrieType::Nominal { arguments, .. }
        | ValkyrieType::Apply(_, arguments)
        | ValkyrieType::TraitObject(crate::compiler::types::hir::TraitObject { type_arguments: arguments, .. }) => {
            for argument in arguments {
                collect_nominal_declarations(argument, out);
            }
        }
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => collect_nominal_declarations(element, out),
        ValkyrieType::FixedArray { element, .. } => collect_nominal_declarations(element, out),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
            for item in items {
                collect_nominal_declarations(item, out);
            }
        }
        ValkyrieType::Function(function) => {
            for parameter in &function.params {
                collect_nominal_declarations(parameter, out);
            }
            collect_nominal_declarations(&function.return_type, out);
        }
        ValkyrieType::TypeLambda(lambda) => collect_nominal_declarations(&lambda.body, out),
        ValkyrieType::Associated(associated) => {
            collect_nominal_declarations(&associated.base, out);
            for argument in &associated.type_arguments {
                collect_nominal_declarations(argument, out);
            }
        }
        ValkyrieType::Row(row) => {
            for method in &row.methods {
                for parameter in &method.params {
                    collect_nominal_declarations(parameter, out);
                }
                collect_nominal_declarations(&method.return_type, out);
            }
        }
        _ => {}
    }
}
