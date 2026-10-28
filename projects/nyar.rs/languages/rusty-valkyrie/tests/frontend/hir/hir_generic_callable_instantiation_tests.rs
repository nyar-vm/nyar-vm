//! 泛型 callable 单态化：`imply X<T>` 成员的实例只由调用点代入产生。

use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::types::hir::{HirExprKind, HirFunction, HirResolvedCall, HirStatementKind},
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
