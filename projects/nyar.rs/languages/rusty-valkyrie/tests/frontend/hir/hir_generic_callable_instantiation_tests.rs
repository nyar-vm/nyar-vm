//! 泛型 callable 单态化：`imply X<T>` 成员的实例只由调用点代入产生。

use crate::primitives::SourceFixtureCompile;
use rusty_valkyrie::{
    CompilerSourceGroup, HirError, ValkyrieCompiler,
    compiler::types::hir::{HirBlock, HirCallResolutionFailureKind, HirExprKind, HirFunction, HirResolvedCall, HirStatementKind},
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

namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::f64")]
structure f64 { }
"#;

fn compile(source: &str) -> rusty_valkyrie::types::hir::HirModule {
    ValkyrieCompiler::default().compile_source(source).expect("泛型 impl 源码必须完成 HIR")
}

/// 递归泛型的更深自代入（`boom<T>` 体内 `boom::<Box<T>>`）会让 `(声明, 代入)`
/// 无界膨胀；单态化固定点必须在轮次上限处 fail-closed，诊断携带增长最快的
/// 声明名，不得静默截断实例表。
///
/// 病态输入的代价本身就是发散的（第 512 轮的克隆体携带 512 层嵌套类型）：
/// 该用例全链跑一次约两分钟，验证的是 fail-closed 诊断形态而非吞吐；
/// 手动跑：`cargo test -- --ignored deeper_self_substitution`。
#[test]
#[ignore = "病态发散输入，按上面对说明手动运行"]
fn deeper_self_substitution_fails_closed_at_fixpoint_round_limit() {
    let source = r#"
class Box<T> {
    _value: T
}

micro boom<T>(x: T) -> i32 {
    return boom::<Box<T>>(Box { _value: x })
}

[export]
micro probe() -> i32 {
    return boom::<i32>(1)
}
"#;
    let failure = ValkyrieCompiler::default()
        .compile_fixture_source(source)
        .expect_err("不收敛的固定点必须结构化失败");
    let HirError::GenericInstantiationLimitExceeded { kind, limit, top_growth, .. } = failure
    else {
        panic!("必须是单态化上限诊断: {failure}");
    };
    assert_eq!(kind, rusty_valkyrie::compiler::hir::lowering::errors::GenericInstantiationLimitKind::Rounds);
    assert_eq!(limit, rusty_valkyrie::compiler::hir::lowering::MAX_FIXPOINT_ROUNDS);
    assert!(top_growth.iter().any(|(name, _)| name.ends_with("boom")), "top-N 必须含增长声明: {top_growth:?}");
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

/// 深遍历函数体全部已解析调用，覆盖聚合字面量字段里的调用。
fn deep_body_calls(function: &HirFunction) -> Vec<&HirResolvedCall> {
    fn walk_block<'a>(block: &'a HirBlock, calls: &mut Vec<&'a HirResolvedCall>) {
        for statement in &block.statements {
            match &statement.kind {
                HirStatementKind::Let { initializer: Some(initializer), .. } => walk_expr(initializer, calls),
                HirStatementKind::Expr(expr) => walk_expr(expr, calls),
                _ => {}
            }
        }
        if let Some(expr) = &block.expr {
            walk_expr(expr, calls);
        }
    }
    fn walk_expr<'a>(expr: &'a rusty_valkyrie::compiler::types::hir::HirExpr, calls: &mut Vec<&'a HirResolvedCall>) {
        match &expr.kind {
            HirExprKind::Call { callee, resolved, args } => {
                if let Some(resolved) = resolved {
                    calls.push(resolved);
                }
                walk_expr(callee, calls);
                for arg in args {
                    walk_expr(&arg.value, calls);
                }
            }
            HirExprKind::Construct { args, .. } => {
                for arg in args {
                    walk_expr(arg, calls);
                }
            }
            HirExprKind::FieldInit { value, .. } => walk_expr(value, calls),
            HirExprKind::Block(block) => walk_block(block, calls),
            HirExprKind::Return(Some(inner)) | HirExprKind::Yield(Some(inner)) => walk_expr(inner, calls),
            HirExprKind::If { condition, then_branch, else_branch, .. } => {
                walk_expr(condition, calls);
                walk_block(then_branch, calls);
                if let Some(branch) = else_branch {
                    walk_block(branch, calls);
                }
            }
            HirExprKind::IfLet { scrutinee, then_branch, else_branch, .. } => {
                walk_expr(scrutinee, calls);
                walk_block(then_branch, calls);
                if let Some(branch) = else_branch {
                    walk_block(branch, calls);
                }
            }
            _ => {}
        }
    }
    let mut calls = Vec::new();
    walk_block(&function.body, &mut calls);
    calls
}

/// 同一 trait 在两个 owner 上各有一个同名成员，泛型接收者实例化后必须按 owner 选中。
const TWO_OWNER_TRAIT_SOURCE: &str = r#"
namespace test.witness;

[primitive("std::primitive::i32")]
structure i32 { }

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

/// `imply Box<U>: Echo` 的 `U` 没有声明 binder（impl 头部只把 owner 类自身的泛型参数
/// 绑进 `generics`），所以它不是 `Box<i32>` 上的第二个可求证据的 impl 身份。
/// 调用点只能按 owner 实例选中 `imply Box<T>: Echo`，这不是同特异性多匹配。
///
/// 依据 01 §4：impl 候选身份由完整头部（generic binders、target、关联等式）决定；
/// 特异性排序与歧义失败只发生在**实际匹配**的候选之间。此前该测试在 `cd564617`
/// 变绿，是因为 `discharge_trait_witness` 尚无「witness 冻结在 impl 成员声明上」的
/// 身份形态，任何 impl 成员 witness 都必然不闭合；`3be47f26` 补齐该形态后，
/// 唯一 owner 匹配候选被正常消去。
const AMBIGUOUS_OWNER_TRAIT_SOURCE: &str = r#"
namespace test.witness;

[primitive("std::primitive::i32")]
structure i32 { }

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
fn binderless_owner_generic_adds_no_second_witness_candidate() {
    // `imply Box<U>: Echo` 的 `U` 没有声明 binder，不能对 `Box<i32>` 求证据；
    // 因此这里只有一个按 owner 实例匹配的合法候选，不构成同特异性多匹配。
    let hir = compile(AMBIGUOUS_OWNER_TRAIT_SOURCE);
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
        let instance = call.instance.expect("唯一 owner 匹配候选必须被选中");
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
    assert_eq!(selected.len(), 1, "`Box<U>` 没有 binder，不得成为第二个 witness 候选: {selected:?}");
    for callable in &hir.instantiated_callables {
        if let Some(owner) = callable.owner_type.as_ref() {
            assert!(!format!("{owner:?}").contains("binder: None"), "binder-less owner 不得铸造实例: {owner:?}");
        }
    }
}

/// 静态 trait 方法没有接收者槽：owner 只能由 witness 冻结的 self 类型约束，
/// 不能靠 `parameter_types` 里的 self 实参隐式统一。
const STATIC_TRAIT_SOURCE: &str = r#"
namespace test.witness;

[primitive("std::primitive::i32")]
structure i32 { }

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

[primitive("std::primitive::i32")]
structure i32 { }

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
    let groups = crate::primitives::groups_with_primitives(vec![
        CompilerSourceGroup::single_source("lib", "lib", CROSS_NAMESPACE_LIBRARY),
        CompilerSourceGroup::single_source("app", "app", CROSS_NAMESPACE_APP).as_entry().with_direct_dependencies(vec!["lib".into()]),
    ]);
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

/// 具体接收者直接调用泛型 impl 的 trait 成员（`ArrayList<u8>` 上 `⁅ ⁆` 这类形态）：
/// 解析期重载决胜已选定 impl 成员，witness 冻结的是成员自身声明而不是 trait 方法槽；
/// 固定点必须承认这种身份并闭合代入、铸造实例，否则 witness 存活到后端计划层失败。
const CONCRETE_RECEIVER_TRAIT_CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

namespace std.operator;

trait Length {
    micro length(self): i64
}

namespace std.collection;

class ArrayList<T> {
    _length: i64
}

imply ArrayList<T> {
    micro new(capacity: i64): Self {
        return ArrayList { _length: capacity }
    }
}

imply ArrayList<T>: std.operator::Length {
    micro length(self): i64 {
        return self._length
    }
}
"#;

const CONCRETE_RECEIVER_TRAIT_APP: &str = r#"
namespace demo;

using std.collection::{ArrayList}
using std.operator::{i64}

[export]
micro probe() -> i64 {
    let list: ArrayList<i64> = ArrayList::new(7)
    return list.length()
}
"#;

#[test]
fn concrete_receiver_trait_member_witness_discharges_to_selected_impl_instance() {
    let groups = [
        CompilerSourceGroup::single_source("core", "core", CONCRETE_RECEIVER_TRAIT_CORE),
        CompilerSourceGroup::single_source("app", "app", CONCRETE_RECEIVER_TRAIT_APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).expect("跨包泛型 impl 必须完成 HIR");
    let core = modules
        .iter()
        .find(|module| module.traits.iter().any(|item| item.name.as_str() == "Length"))
        .expect("core 模块必须含 `Length` trait");
    let length_trait = core.traits.iter().find(|item| item.name.as_str() == "Length").and_then(|item| item.declaration).expect("trait 声明身份");
    let impl_member = core
        .impls
        .iter()
        .find(|implementation| implementation.trait_declaration == Some(length_trait))
        .and_then(|implementation| implementation.methods.iter().find(|method| method.name.as_str() == "length"))
        .and_then(|method| method.declaration)
        .expect("`imply ArrayList<T>: Length` 成员声明身份");
    let app = modules
        .iter()
        .find(|module| module.functions.iter().any(|function| function.name.as_str() == "probe"))
        .expect("app 模块必须含 `probe`");
    let probe = app.functions.iter().find(|function| function.name.as_str() == "probe").expect("probe");
    let calls = body_calls(probe);
    let call = calls.iter().copied().find(|call| call.symbol.parts().last().is_some_and(|part| part.as_str() == "length")).expect("length 调用");
    assert!(call.trait_dispatch.is_none(), "具体接收者的 witness 必须由固定点消去: {:?}", call.trait_dispatch);
    let instance = call.instance.expect("消去 witness 后调用点必须指向具体 impl 实例");
    let member = modules
        .iter()
        .flat_map(|module| module.instantiated_callables.iter())
        .find(|callable| callable.function.instance == Some(instance))
        .expect("被选 impl 成员的实例必须由固定点铸造");
    assert_eq!(member.function.declaration, Some(impl_member), "实例必须来自解析期选定的那个 impl 成员声明");
    assert_eq!(member.trait_declaration, Some(length_trait), "实例必须保留 trait 归属");
    let owner = member.owner_type.as_ref().expect("impl 实例必须携带具体 owner");
    assert_closed(owner, "实例 owner 类型");
}

/// 同一 impl 泛型成员之间的调用链（`echo` → `value`）：声明体按开放 binder 铸出的
/// 占位实例不得被具体克隆体继承，具体克隆体必须按自己的代入重铸并指向具体实例。
const CHAINED_IMPL_MEMBER_SOURCE: &str = r#"
namespace test.instance;

[primitive("std::primitive::i32")]
structure i32 { }

class Box<T> {
    _value: T
}

imply Box<T> {
    micro new(value: T): Self {
        return Box { _value: value }
    }

    micro value(self) -> T {
        return self._value
    }

    micro echo(self) -> T {
        let inner: T = self.value()
        return inner
    }
}

[export]
micro probe() -> i32 {
    let boxed: Box<i32> = Box.new(7)
    let result: i32 = boxed.echo()
    return result
}
"#;

#[test]
fn monomorphized_clone_member_call_targets_concrete_instance() {
    let hir = compile(CHAINED_IMPL_MEMBER_SOURCE);
    let echoes = hir
        .instantiated_callables
        .iter()
        .filter(|callable| callable.function.name.as_str() == "echo")
        .collect::<Vec<_>>();
    assert_eq!(echoes.len(), 1, "`Box<i32>` 只应有一个 `echo` 实例");
    let clone = echoes[0];
    let receiver = clone.function.params.first().map(|param| param.ty.clone()).expect("`echo` 必须有接收者实参");
    assert_closed(&receiver, "`echo` 实例接收者");

    let calls = body_calls(&clone.function);
    let call = calls
        .iter()
        .copied()
        .find(|call| call.symbol.parts().last().is_some_and(|part| part.as_str() == "value"))
        .expect("`echo` 实例体内必须保留 `value` 调用");
    assert!(call.trait_dispatch.is_none(), "普通 impl 成员调用不得携带 witness");
    let instance = call.instance.expect("克隆体内的 impl 泛型成员调用必须已闭合");
    let member = hir
        .instantiated_callables
        .iter()
        .find(|callable| callable.function.instance == Some(instance))
        .expect("被调用成员的实例必须已由固定点铸造");
    assert_eq!(member.function.name.as_str(), "value", "只能指向 `value` 成员");
    assert_eq!(member.owner_type.as_ref(), Some(&receiver), "克隆体必须指向 `Box<i32>` 实例，而不是声明体的开放占位实例");
    assert_closed(member.owner_type.as_ref().expect("`value` 实例 owner"), "`value` 实例 owner");
}

/// 反例：`Box::read` 的 `where T: Marked` 在 `i32` 上没有证据，具体代入不得铸造实例；
/// 克隆体内的该调用必须保持 `instance = None` 并由合同验证报 `InstanceNotClosed`。
const UNCLOSED_WHERE_INSTANCE_SOURCE: &str = r#"
namespace test.instance;

[primitive("std::primitive::i32")]
structure i32 { }

trait Marked {
    micro mark(self) -> bool
}

class Box<T> {
    _value: T
}

imply Box<T> {
    micro new(value: T): Self {
        return Box { _value: value }
    }

    micro read(self) -> T
        where T: Marked
    {
        return self._value
    }
}

class Wrapper<T> {
    _boxed: Box<T>
}

imply Wrapper<T> {
    micro read(self) -> T {
        let value: T = self._boxed.read()
        return value
    }
}

[export]
micro probe() -> i32 {
    let boxed: Box<i32> = Box.new(1)
    let wrapper: Wrapper<i32> = Wrapper { _boxed: boxed }
    let result: i32 = wrapper.read()
    return result
}
"#;

#[test]
fn where_rejected_member_instance_stays_unclosed() {
    let error = ValkyrieCompiler::default()
        .compile_source(UNCLOSED_WHERE_INSTANCE_SOURCE)
        .expect_err("`i32` 没有 `Marked` 证据时 `Box::read` 实例不得铸造");
    let HirError::UnresolvedCall { reason, .. } = &error
    else {
        panic!("必须是未闭合调用诊断: {error:?}");
    };
    assert!(
        matches!(reason, HirCallResolutionFailureKind::InstanceNotClosed),
        "缺失 where 证据必须以 `InstanceNotClosed` 失败: {error:?}"
    );
}

const CONTAINER_MEMBER_SOURCE: &str = r#"
namespace test.instance;

[primitive("std::primitive::i64")]
structure i64 { }

class Cell<T> {
}

class Bucket<K, V> {
    keys: Cell<K>
    values: Cell<V>
}

class Table<K, V> {
    _root: Bucket<K, V>
}

imply Cell<T> {
    micro make(): Self {
        return Cell { }
    }
}

imply Table<K, V> {
    micro new(): Self {
        let root: Bucket<K, V> = Bucket {
            keys: Cell.make(),
            values: Cell.make(),
        }
        return Table { _root: root }
    }
}

[export]
micro probe() -> i64 {
    let table: Table<i64, i64> = Table.new()
    return 0
}
"#;

const OPEN_RESIDUAL_SOURCE: &str = r#"
namespace test.instance;

[primitive("std::primitive::i64")]
structure i64 { }

class Cell<T> {
}

class Bucket<K, V> {
    keys: Cell<K>
    values: Cell<V>
}

class Table<K, V> {
    _root: Bucket<K, V>
}

imply Cell<T> {
    micro make(): Self {
        return Cell { }
    }
}

imply Table<K, V> {
    micro new(): Self {
        let root = Bucket {
            keys: Cell.make(),
            values: Cell.make(),
        }
        return Table { _root: root }
    }
}

[export]
micro probe() -> i64 {
    let table: Table<i64, i64> = Table.new()
    return 0
}
"#;

/// 反向合同：克隆体里仍带开放 binder 的 callee 必须在 HIR 边界结构化失败，
/// 点名被调声明与捕获的泛型，不得落成 MIR 占位操作数静默发射。
#[test]
fn open_residual_in_clone_fails_closed_with_captured_binders() {
    let error = ValkyrieCompiler::default()
        .compile_source(OPEN_RESIDUAL_SOURCE)
        .expect_err("无标注聚合字面量留下的开放 binder 必须失败关闭");
    let HirError::UnresolvedCall { selector, context, reason, .. } = &error
    else {
        panic!("必须是未闭合调用诊断: {error:?}");
    };
    assert!(
        matches!(reason, HirCallResolutionFailureKind::InstanceNotClosed),
        "开放残留必须以 `InstanceNotClosed` 失败: {reason:?}"
    );
    assert!(selector.contains("make"), "诊断必须点名被调 callee: {selector}");
    assert!(context.contains("declaration="), "诊断必须点名被调声明: {context}");
    assert!(context.contains("captured="), "诊断必须点名捕获的泛型 binder: {context}");
}

/// 泛型容器的成员体内构造另一个泛型容器：`Table<i64,i64>::new` 体内的
/// `Cell.new` 必须落到 `Cell<i64>::new` 的闭合实例，而不是保留声明期的开放克隆。
#[test]
fn generic_container_member_body_call_lands_on_closed_instance() {
    let hir = compile(CONTAINER_MEMBER_SOURCE);
    let table = hir.structs.iter().find(|structure| structure.name.as_str() == "Table").expect("Table 类");
    let table_new = hir
        .impls
        .iter()
        .filter(|implementation| implementation.target.nominal_declaration() == table.constructor_declaration)
        .flat_map(|implementation| implementation.methods.iter())
        .find(|method| method.name.as_str() == "new")
        .and_then(|method| method.declaration)
        .expect("Table::new 声明身份");
    let clone = hir
        .instantiated_callables
        .iter()
        .find(|callable| callable.function.declaration == Some(table_new))
        .expect("`Table<i64,i64>::new` 必须由固定点铸造");
    assert_closed(&clone.function.return_type, "`Table::new` 实例返回类型");
    let calls = deep_body_calls(&clone.function);
    assert_eq!(calls.len(), 2, "`Table::new` 实例体内必须有两条 `Cell.new` 调用");
    for call in &calls {
        assert_closed(&call.return_type, "体内 `Cell.new` 返回类型");
        for parameter in &call.parameter_types {
            assert_closed(parameter, "体内 `Cell.new` 形参类型");
        }
        let instance = call.instance.expect("闭合的体内 callee 必须落到具体实例");
        let callee = hir
            .instantiated_callables
            .iter()
            .find(|callable| callable.function.instance == Some(instance))
            .expect("被调 `Cell::new` 实例必须随固定点登记");
        assert_closed(&callee.function.return_type, "被调 `Cell::new` 实例返回类型");
        for parameter in &callee.function.params {
            assert_closed(&parameter.ty, "被调 `Cell::new` 实例形参类型");
        }
    }
}
