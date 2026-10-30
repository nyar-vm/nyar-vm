//! 泛型 callable 单态化：`imply X<T>` 成员的实例只由调用点代入产生。

use rusty_valkyrie::{
    CompilerSourceGroup, HirError, ValkyrieCompiler,
    compiler::types::hir::{HirExprKind, HirFunction, HirResolvedCall, HirStatementKind},
    hir::lowering::DEFAULT_COMPILE_ARCH,
    types::hir::ValkyrieType,
};

const BOX_SOURCE: &str = r#"
class Box<T> {
    _value: T
}

imply Box<T> {
    micro new(value: T): Self {
        return Box { _value: value }
    }

    micro get(self) -> T {
        return self._value
    }
}

micro run() -> i32 {
    let first: i32 = 1
    let second: i32 = 2
    let left = Box.new(first)
    let right = Box.new(second)
    let value: i32 = left.get()
    let third: f64 = 2.5
    let scaled = Box.new(third)
    let real: f64 = scaled.get()
    return value
}
"#;

fn compile(source: &str) -> rusty_valkyrie::types::hir::HirModule {
    ValkyrieCompiler::default().compile_source(source).expect("泛型 impl 源码必须完成 HIR")
}

fn resolved_calls(function: &HirFunction) -> Vec<&HirResolvedCall> {
    let mut calls = Vec::new();
    for statement in &function.body.statements {
        let expression = match &statement.kind {
            HirStatementKind::Let { initializer: Some(expression), .. } => expression,
            HirStatementKind::Expr(expression) => expression,
            _ => continue,
        };
        if let HirExprKind::Call { resolved: Some(call), .. } = &expression.kind {
            calls.push(call);
        }
    }
    calls
}

fn run_function(module: &rusty_valkyrie::types::hir::HirModule) -> &HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == "run").expect("run")
}

/// 收集体内的全部已解析调用，包含尾部块表达式里的 `return ...` 调用。
fn body_calls(function: &HirFunction) -> Vec<&HirResolvedCall> {
    let mut calls = resolved_calls(function);
    if let Some(expression) = function.body.expr.as_deref() {
        collect_expr_calls(expression, &mut calls);
    }
    calls
}

fn collect_expr_calls<'a>(expression: &'a rusty_valkyrie::compiler::types::hir::HirExpr, calls: &mut Vec<&'a HirResolvedCall>) {
    match &expression.kind {
        HirExprKind::Return(Some(inner)) | HirExprKind::Yield(Some(inner)) => collect_expr_calls(inner, calls),
        HirExprKind::Call { callee, resolved, args } => {
            if let Some(resolved) = resolved {
                calls.push(resolved);
            }
            collect_expr_calls(callee, calls);
            for arg in args {
                collect_expr_calls(&arg.value, calls);
            }
        }
        _ => {}
    }
}

/// 同一 trait 在两个 owner 上各有一个同名成员，泛型接收者实例化后必须按 owner 选中。
const TWO_OWNER_TRAIT_SOURCE: &str = r#"
namespace test.witness;

trait Echo {
    micro echo(self) -> bool
}

structure Left { _x: i32 }
structure Right { _x: i32 }

imply Left: Echo {
    micro echo(self) -> bool { return false }
}

imply Right: Echo {
    micro echo(self) -> bool { return true }
}

micro use_echo<T>(value: T) -> bool where T: Echo {
    return value.echo()
}

micro caller() -> bool {
    let left = Left { _x: 1 }
    let right = Right { _x: 2 }
    let first: bool = use_echo(left)
    let second: bool = use_echo(right)
    return first
}
"#;

/// 同一 trait 在同一 owner 声明的两个候选 binder 形态上都成立：调用点必须结构化失败。
const AMBIGUOUS_OWNER_TRAIT_SOURCE: &str = r#"
namespace test.witness;

trait Echo {
    micro echo(self) -> bool
}

class Box<T> {
    _value: T
}

imply Box<T>: Echo {
    micro echo(self) -> bool { return false }
}

imply Box<U>: Echo {
    micro echo(self) -> bool { return true }
}

micro use_echo<T>(value: Box<T>) -> bool where Box<T>: Echo {
    return value.echo()
}

micro caller() -> bool {
    let boxed: Box<i32> = Box { _value: 1 }
    let value: bool = use_echo(boxed)
    return value
}
"#;

#[test]
fn trait_dispatch_parameter_types_carry_receiver_as_first_argument() {
    // 实测结论：trait dispatch 的已解析合同把接收者放在 `parameter_types[0]`，
    // 因此 owner 在 `derive_substitution` 里已经隐式参与统一。显式 owner 比较仍然必要：
    // 它还要求 owner binder 闭合（`parameter_types` 只是按调用点推断，不保证合并后的
    // 代入覆盖 owner 全部 binder），并覆盖没有接收者槽的分派面。
    let hir = compile(TWO_OWNER_TRAIT_SOURCE);
    let use_echo = hir.functions.iter().find(|function| function.name.as_str() == "use_echo").expect("use_echo");
    let calls = body_calls(use_echo);
    assert_eq!(calls.len(), 1, "`use_echo` 只剩一处 trait 方法调用");
    let call = calls[0];
    assert!(call.instance.is_none(), "开放泛型 witness 不得伪造可执行实例");
    let dispatch = call.trait_dispatch.as_ref().expect("必须保留 trait 分派义务");
    assert!(call.has_receiver, "trait 方法调用必须声明接收者槽");
    assert_eq!(call.parameter_types.len(), 1, "接收者必须作为第 0 个实参出现在合同里");
    assert_eq!(call.parameter_types[0], dispatch.self_type, "`parameter_types[0]` 必须就是 witness 冻结的 self 类型");
}

#[test]
fn trait_witness_discharges_to_owner_matched_impl_instance() {
    let hir = compile(TWO_OWNER_TRAIT_SOURCE);
    let echo_trait = hir.traits.iter().find(|item| item.name.as_str() == "Echo").expect("Echo 声明");
    let trait_declaration = echo_trait.declaration.expect("trait 声明身份");
    let use_echo = hir
        .functions
        .iter()
        .find(|function| function.name.as_str() == "use_echo")
        .and_then(|function| function.declaration)
        .expect("use_echo 声明身份");

    let mut selected = Vec::new();
    for callable in hir.instantiated_callables.iter().filter(|callable| callable.function.declaration == Some(use_echo)) {
        let receiver = callable.function.params.first().map(|param| param.ty.clone()).expect("`use_echo` 必须有接收者实参");
        assert_closed(&receiver, "实例化接收者类型");
        let calls = body_calls(&callable.function);
        assert_eq!(calls.len(), 1, "实例体内必须只有一处 trait 方法调用: {calls:?}");
        let call = calls[0];
        assert!(call.trait_dispatch.is_none(), "接收者实例身份已确定时必须消去 witness: {:?}", call.trait_dispatch);
        let instance = call.instance.expect("消去 witness 后调用点必须指向具体 impl 实例");
        let member = hir
            .instantiated_callables
            .iter()
            .find(|callable| callable.function.instance == Some(instance))
            .expect("impl 实例必须已由固定点铸造");
        assert_eq!(member.function.name.as_str(), "echo");
        assert_eq!(member.trait_declaration, Some(trait_declaration), "只能选中同一 trait 的 impl 实例");
        assert_eq!(member.owner_type.as_ref(), Some(&receiver), "impl owner 必须与接收者实例身份一致");
        selected.push(instance);
    }
    assert_eq!(selected.len(), 2, "`Left` 与 `Right` 两个接收者实例各需选中各自的 impl");
    selected.sort();
    selected.dedup();
    assert_eq!(selected.len(), 2, "不同 owner 不得复用同一 impl 实例");
}

#[test]
fn ambiguous_owner_witness_is_rejected_instead_of_picking_a_candidate() {
    // `imply Box<T>: Echo` 与 `imply Box<U>: Echo` 对同一个 self 类型给出两个候选：
    // 调用点必须结构化失败，禁止静默取第一个候选，也不得让 witness 漂到后端再猜。
    let error = ValkyrieCompiler::default()
        .compile_source(AMBIGUOUS_OWNER_TRAIT_SOURCE)
        .expect_err("同一 owner 的多个候选 witness 必须在调用点边界失败");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "多个候选 witness 必须结构化失败，不得回退首个实现: {error:?}");
}

/// 静态 trait 方法没有接收者槽：owner 只能由 witness 冻结的 self 类型约束，
/// 不能靠 `parameter_types` 里的 self 实参隐式统一。
const STATIC_TRAIT_SOURCE: &str = r#"
namespace test.witness;

trait Tagged {
    micro tag() -> bool
}

structure Left { _x: i32 }
structure Right { _x: i32 }

imply Left: Tagged {
    micro tag() -> bool { return false }
}

imply Right: Tagged {
    micro tag() -> bool { return true }
}

micro use_tag<T>(value: T) -> bool where T: Tagged {
    let unused = value
    return T::tag()
}

micro caller() -> bool {
    let left = Left { _x: 1 }
    let right = Right { _x: 2 }
    let first: bool = use_tag(left)
    let second: bool = use_tag(right)
    return first
}
"#;

/// 同一 trait 只在另一个 owner 上有实现：调用点必须结构化失败，
/// 不得借用别处 owner 的 witness 让程序通过。
const MISSING_WITNESS_SOURCE: &str = r#"
namespace test.witness;

trait Tagged {
    micro tag() -> bool
}

structure Left { _x: i32 }
structure Right { _x: i32 }

imply Right: Tagged {
    micro tag() -> bool { return true }
}

micro use_tag<T>(value: T) -> bool where T: Tagged {
    let unused = value
    return T::tag()
}

micro caller(value: Left) -> bool {
    return use_tag(value)
}
"#;

/// 依赖组：命名空间 `lib` 自己的 `Echo`，并在 `lib::Value` 上有一个返回值不同的实现。
const CROSS_NAMESPACE_LIBRARY: &str = r#"
namespace lib;

trait Echo {
    micro echo(self) -> i64
}

structure Value { _x: i32 }

imply Value: Echo {
    micro echo(self) -> i64 { return 1 }
}
"#;

/// 消费组：命名空间 `app` 也有同名 trait `Echo`，并按自己的 trait 身份实现 `app::Value`。
const CROSS_NAMESPACE_APP: &str = r#"
namespace app;

trait Echo {
    micro echo(self) -> i64
}

structure Value { _x: i32 }

imply Value: Echo {
    micro echo(self) -> i64 { return 2 }
}

micro use_echo<T>(value: T) -> i64 where T: Echo {
    return value.echo()
}

[export] micro caller(value: Value) -> i64 {
    return use_echo(value)
}
"#;

#[test]
fn static_trait_witness_discharges_to_owner_matched_impl_instance() {
    let hir = compile(STATIC_TRAIT_SOURCE);
    let tagged = hir.traits.iter().find(|item| item.name.as_str() == "Tagged").and_then(|item| item.declaration).expect("Tagged 声明身份");
    let use_tag = hir
        .functions
        .iter()
        .find(|function| function.name.as_str() == "use_tag")
        .and_then(|function| function.declaration)
        .expect("use_tag 声明身份");

    let mut selected = Vec::new();
    for callable in hir.instantiated_callables.iter().filter(|callable| callable.function.declaration == Some(use_tag)) {
        let receiver = callable.function.params.first().map(|param| param.ty.clone()).expect("`use_tag` 必须有接收者实参");
        assert_closed(&receiver, "实例化接收者类型");
        let calls = body_calls(&callable.function);
        assert_eq!(calls.len(), 1, "实例体内必须只有一处静态 trait 方法调用: {calls:?}");
        let call = calls[0];
        assert!(!call.has_receiver, "静态 trait 方法没有接收者槽，owner 只能由 self 类型约束");
        assert!(call.trait_dispatch.is_none(), "self 类型具体化后必须消去 witness: {:?}", call.trait_dispatch);
        let instance = call.instance.expect("静态 witness 也必须落到具体 impl 实例");
        let member = hir
            .instantiated_callables
            .iter()
            .find(|callable| callable.function.instance == Some(instance))
            .expect("impl 实例必须已由固定点铸造");
        assert_eq!(member.function.name.as_str(), "tag");
        assert_eq!(member.trait_declaration, Some(tagged), "只能选中同一 trait 的 impl 实例");
        assert_eq!(member.owner_type.as_ref(), Some(&receiver), "静态 witness 必须按 self 类型选中 owner 一致的 impl");
        selected.push(instance);
    }
    assert_eq!(selected.len(), 2, "`Left` 与 `Right` 两个静态分派实例各需选中各自的 impl");
    selected.sort();
    selected.dedup();
    assert_eq!(selected.len(), 2, "不同 owner 不得复用同一 impl 实例");
}

#[test]
fn missing_witness_fails_at_call_site_instead_of_borrowing_another_owner() {
    let error =
        ValkyrieCompiler::default().compile_source(MISSING_WITNESS_SOURCE).expect_err("`Left` 没有 `Tagged` 实现时调用点必须结构化失败");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "缺失 witness 必须在调用点边界失败，不得借用 `Right` 的实现: {error:?}");
}

#[test]
fn cross_namespace_same_trait_name_does_not_provide_witness() {
    let groups = [
        CompilerSourceGroup::single_source("lib", "lib", CROSS_NAMESPACE_LIBRARY),
        CompilerSourceGroup::single_source("app", "app", CROSS_NAMESPACE_APP).as_entry().with_direct_dependencies(vec!["lib".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).expect("两个命名空间必须完成 HIR");
    let app = modules
        .iter()
        .find(|module| module.functions.iter().any(|function| function.name.as_str() == "use_echo"))
        .expect("`use_echo` 必须在 `app` 模块里");
    let app_trait = app.traits.iter().find(|item| item.name.as_str() == "Echo").and_then(|item| item.declaration).expect("app::Echo 声明身份");
    let app_impl_member = app
        .impls
        .iter()
        .find(|implementation| implementation.trait_declaration == Some(app_trait))
        .and_then(|implementation| implementation.methods.iter().find(|method| method.name.as_str() == "echo"))
        .and_then(|method| method.declaration)
        .expect("app::Echo 的 `i64` 实现成员");
    let use_echo = app
        .functions
        .iter()
        .find(|function| function.name.as_str() == "use_echo")
        .and_then(|function| function.declaration)
        .expect("use_echo 声明身份");

    let callable = app
        .instantiated_callables
        .iter()
        .find(|callable| callable.function.declaration == Some(use_echo) && callable.owner_type.is_none())
        .expect("`use_echo` 的 `i64` 实例");
    let calls = body_calls(&callable.function);
    assert_eq!(calls.len(), 1, "`use_echo` 实例体内必须只有一处 trait 方法调用");
    let call = calls[0];
    assert!(call.trait_dispatch.is_none(), "self 类型具体化后必须消去 witness");
    let instance = call.instance.expect("必须落到具体 impl 实例");
    let member =
        app.instantiated_callables.iter().find(|callable| callable.function.instance == Some(instance)).expect("impl 实例必须已由固定点铸造");
    assert_eq!(
        member.function.declaration,
        Some(app_impl_member),
        "只能选择同一 namespace 里 `app::Echo` 的实现，不得借用 `lib` 同名 trait 的实现"
    );
    assert_eq!(member.trait_declaration, Some(app_trait), "witness 必须保留原 trait 声明身份");
}

/// 实例签名、局部与聚合类型不得残留未代入 binder；用 HIR 类型自身的结构化打印判定，
/// 避免在测试里按类型分支重写一遍遍历。
fn assert_closed(ty: &ValkyrieType, context: &str) {
    let rendered = format!("{ty:?}");
    assert!(!rendered.contains("binder: Some("), "{context} 仍含开放泛型 binder: {rendered}");
}

#[test]
fn generic_impl_member_has_no_declaration_time_instance() {
    let hir = compile(BOX_SOURCE);

    for implementation in &hir.impls {
        for method in &implementation.methods {
            assert!(!method.is_abstract);
            assert!(method.generics.is_empty(), "实例声明不得再携带开放 binder: {}", method.name.as_str());
            assert!(method.instance.is_none(), "`imply Box<T>` 成员不得保留声明期种子实例: {}", method.name.as_str());
            assert!(method.declaration.is_some(), "成员仍必须有声明身份: {}", method.name.as_str());
        }
    }
    assert!(hir.instantiated_callables.is_empty() || !hir.impls.is_empty());
}

#[test]
fn call_sites_point_at_instances_and_instances_are_closed() {
    let hir = compile(BOX_SOURCE);
    let calls = resolved_calls(run_function(&hir));
    assert_eq!(calls.len(), 5, "源码里的五个泛型成员调用都必须完成 HIR 解析");

    let instance_ids = calls.iter().map(|call| call.instance.expect("调用点必须指向真实实例")).collect::<Vec<_>>();
    for call in &calls {
        assert_closed(&call.return_type, "调用点返回类型");
        for parameter in &call.parameter_types {
            assert_closed(parameter, "调用点形参类型");
        }
    }
    // 同一声明 + 同一具体代入必须复用同一实例（固定点收敛）。
    assert_eq!(instance_ids[0], instance_ids[1], "`Box.new(i32)` 的两次调用必须落在同一实例上");
    // 不同代入必须产生不同实例。
    assert_ne!(instance_ids[0], instance_ids[3], "`Box.new(i32)` 与 `Box.new(f64)` 必须是不同实例");
    assert_ne!(instance_ids[2], instance_ids[4], "`get(i32)` 与 `get(f64)` 必须是不同实例");
    assert_ne!(instance_ids[0], instance_ids[2], "同一 owner 的不同成员必须是不同实例");

    let mut emitted = hir.instantiated_callables.iter().filter_map(|callable| callable.function.instance).collect::<Vec<_>>();
    emitted.sort();
    emitted.dedup();
    let mut used = instance_ids.clone();
    used.sort();
    used.dedup();
    assert_eq!(emitted, used, "调用点引用的实例与已铸造实例必须一一对应");
}

#[test]
fn instantiated_bodies_carry_concrete_types() {
    let hir = compile(BOX_SOURCE);

    for callable in &hir.instantiated_callables {
        let function = &callable.function;
        assert!(function.instance.is_some());
        assert!(function.generics.is_empty(), "实例不得再携带开放泛型参数: {}", function.name.as_str());
        assert!(function.declaration.is_some());
        assert_closed(&function.return_type, "实例返回类型");
        for param in &function.params {
            assert_closed(&param.ty, "实例形参类型");
        }
        let owner = callable.owner_type.as_ref().expect("impl 实例必须携带具体 owner");
        assert_closed(owner, "实例 owner 类型");
        for statement in &function.body.statements {
            if let HirStatementKind::Let { ty: Some(ty), .. } = &statement.kind {
                assert_closed(ty, "实例局部变量类型");
            }
        }
    }
    assert!(!hir.instantiated_callables.is_empty(), "调用点必须铸造实例");
}

#[test]
fn mir_emits_only_closed_callable_instances() {
    use rusty_valkyrie::compiler::mir::MirLowerer;

    let hir = compile(BOX_SOURCE);
    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(mir.diagnostics.is_empty(), "闭合实例不得再产生 callable 身份诊断: {:?}", mir.diagnostics);

    for callable in &hir.instantiated_callables {
        let instance = callable.function.instance.expect("实例身份");
        let function = mir.functions.iter().find(|function| function.instance == Some(instance)).expect("实例必须发射到 MIR");
        for ty in function.param_types.iter().chain(std::iter::once(&function.return_type)) {
            assert_closed(ty, "MIR 实例签名");
        }
        assert!(
            function.display_symbol.contains(&format!("#{}", instance.index())),
            "同一声明的不同代入必须有不同的 ABI 标签: {}",
            function.display_symbol
        );
    }
    // 开放声明自身只允许以「调用点代入铸造的实例」进入 MIR，不得再有声明期种子。
    let open_declarations = hir
        .impls
        .iter()
        .flat_map(|implementation| implementation.methods.iter().filter_map(|method| method.declaration))
        .collect::<std::collections::BTreeSet<_>>();
    let known = hir.instantiated_callables.iter().filter_map(|callable| callable.function.instance).collect::<std::collections::BTreeSet<_>>();
    for function in &mir.functions {
        let Some(instance) = function.instance
        else {
            continue;
        };
        if known.contains(&instance) {
            continue;
        }
        let is_open_member = function.declaration.is_some_and(|declaration| open_declarations.contains(&declaration));
        assert!(!is_open_member, "开放声明不得以声明期种子实例进入 MIR: {}", function.display_symbol);
    }
}

#[test]
fn instantiation_is_deterministic_across_compilations() {
    let first = compile(BOX_SOURCE);
    let second = compile(BOX_SOURCE);

    let projected = |module: &rusty_valkyrie::types::hir::HirModule| {
        module
            .instantiated_callables
            .iter()
            .map(|callable| (callable.function.name.clone(), callable.function.instance, callable.owner_type.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(projected(&first), projected(&second), "同一输入的实例空间必须稳定");
}
