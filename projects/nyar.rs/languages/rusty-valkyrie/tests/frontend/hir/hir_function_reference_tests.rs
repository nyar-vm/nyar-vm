//! 具名函数作值：裸名实参唯一命中非泛型自由函数时冻结为 `FunctionReference`。

use rusty_valkyrie::{
    CompilerSourceGroup, HirError, ValkyrieCompiler,
    compiler::types::hir::{HirExprKind, HirFunction, HirFunctionReference, HirResolvedCall},
    hir::lowering::DEFAULT_COMPILE_ARCH,
    types::hir::ValkyrieType,
};

fn compile(source: &str) -> rusty_valkyrie::types::hir::HirModule {
    ValkyrieCompiler::default().compile_source(source).expect("函数作值源码必须完成 HIR")
}

fn function<'a>(module: &'a rusty_valkyrie::types::hir::HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).expect(name)
}

/// 找到 `run` 体内对 `apply`/`choose` 的调用合同与实参表达式。
fn outer_call(function: &HirFunction) -> (&HirResolvedCall, &rusty_valkyrie::compiler::types::hir::HirExpr) {
    let expression = match &function.body.expr.as_deref().expect("run 体尾表达式").kind {
        HirExprKind::Return(Some(expression)) => expression,
        kind => panic!("期望 return 表达式: {kind:?}"),
    };
    let HirExprKind::Call { args, resolved, .. } = &expression.kind else {
        panic!("期望调用表达式: {:?}", expression.kind)
    };
    let resolved = resolved.as_ref().expect("调用必须完成重载解析");
    (resolved, &args.last().expect("最后一个实参").value)
}

fn reference(expression: &rusty_valkyrie::compiler::types::hir::HirExpr) -> &HirFunctionReference {
    let HirExprKind::FunctionReference(reference) = &expression.kind else {
        panic!("期望 FunctionReference: {:?}", expression.kind)
    };
    reference
}

const GENERIC_CONSUMER_SOURCE: &str = r#"
micro cmp_i64(a: i64, b: i64) -> i32 {
    return 0
}

micro choose<T>(a: T, b: T, cmp: micro(T, T) -> i32) -> i32 {
    return cmp(a, b)
}

micro run() -> i32 {
    let x: i64 = 3
    let y: i64 = 5
    return choose(x, y, cmp_i64)
}
"#;

const NON_GENERIC_CONSUMER_SOURCE: &str = r#"
micro cmp_i32(a: i32, b: i32) -> i32 {
    return 0
}

micro apply(a: i32, b: i32, cmp: micro(i32, i32) -> i32) -> i32 {
    return cmp(a, b)
}

micro run() -> i32 {
    return apply(3, 5, cmp_i32)
}
"#;

const GENERIC_REFERENCE_REJECTED_SOURCE: &str = r#"
micro choose<T>(a: T, b: T, cmp: micro(T, T) -> i32) -> i32 {
    return cmp(a, b)
}

micro apply(f: micro(i64, i64) -> i32) -> i32 {
    return f(1, 2)
}

micro run() -> i32 {
    return apply(choose)
}
"#;

const LOCAL_SHADOW_SOURCE: &str = r#"
micro cmp_i64(a: i64, b: i64) -> i32 {
    return 0
}

micro apply(a: i64, b: i64, cmp: micro(i64, i64) -> i32) -> i32 {
    return cmp(a, b)
}

micro run(cmp: micro(i64, i64) -> i32) -> i32 {
    return apply(3, 5, cmp)
}
"#;

#[test]
fn bare_name_argument_freezes_declaration_and_signature() {
    let hir = compile(NON_GENERIC_CONSUMER_SOURCE);
    let run = function(&hir, "run");
    let (resolved, argument) = outer_call(run);
    let reference = reference(argument);
    let cmp_declaration = function(&hir, "cmp_i32").declaration.expect("cmp_i32 声明身份");
    assert_eq!(reference.declaration, Some(cmp_declaration), "引用必须冻结被引用函数的声明身份");
    assert_eq!(reference.symbol.to_string(), "main.cmp_i32");
    // 引用的冻结签名必须与调用合同闭合出的函数类型形参逐项一致。
    let expected = match resolved.parameter_types.last() {
        Some(ValkyrieType::Function(function)) => function,
        other => panic!("调用合同必须携带函数类型形参: {other:?}"),
    };
    assert_eq!(&reference.signature, expected, "引用签名必须就是调用合同闭合的函数类型");
    assert_eq!(reference.signature.params.len(), 2);
    assert_eq!(reference.signature.return_type, expected.return_type);
}

#[test]
fn function_reference_instance_is_minted_by_instantiation_fixpoint() {
    let hir = compile(NON_GENERIC_CONSUMER_SOURCE);
    let run = function(&hir, "run");
    let (_, argument) = outer_call(run);
    let instance = reference(argument).instance.expect("固定点必须为函数作值铸造实例");
    let cmp_declaration = function(&hir, "cmp_i32").declaration.expect("cmp_i32 声明身份");
    let callable = hir
        .instantiated_callables
        .iter()
        .find(|callable| callable.function.instance == Some(instance))
        .expect("引用实例必须进入已铸造实例表");
    assert_eq!(callable.function.declaration, Some(cmp_declaration), "引用实例必须来自被引用声明");
}

#[test]
fn generic_binder_is_bound_from_function_reference_signature() {
    // `choose<T>` 的 T 只能由 `cmp_i64` 的冻结签名 `micro(i64, i64) -> i32` 绑定；
    // 若 `unify_call_type_binders` 缺 Function 分支，调用会失配并保持 unresolved。
    let hir = compile(GENERIC_CONSUMER_SOURCE);
    let run = function(&hir, "run");
    let (resolved, argument) = outer_call(run);
    let reference = reference(argument);
    assert_eq!(reference.symbol.to_string(), "main.cmp_i64");
    // `choose<T>` 的 T 由 `cmp_i64` 的冻结签名与实参 `x: i64` 共同闭合：
    // 合同第 3 项必须是与引用签名一致的函数类型。
    let expected = match resolved.parameter_types.get(2) {
        Some(ValkyrieType::Function(function)) => function,
        other => panic!("调用合同必须携带函数类型形参: {other:?}"),
    };
    assert_eq!(&reference.signature, expected);
    assert_eq!(resolved.parameter_types.len(), 3);
    assert_eq!(resolved.return_type, expected.return_type);
    let instance = reference.instance.expect("固定点必须铸造实例");
    assert!(hir.instantiated_callables.iter().any(|callable| callable.function.instance == Some(instance)));
}

#[test]
fn generic_function_as_value_stays_unresolved_instead_of_guessing_instantiation() {
    // 泛型声明没有唯一代入：不改写、不铸造，调用沿既有路径结构化失败。
    let error = ValkyrieCompiler::default()
        .compile_source(GENERIC_REFERENCE_REJECTED_SOURCE)
        .expect_err("泛型函数作值不得按短名猜一个代入");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "必须保持 UnresolvedCall fail-closed: {error:?}");
}

#[test]
fn local_binding_shadows_free_function_name() {
    // 与参数同名的裸名是局部绑定：保持 `Variable`，不参与函数作值改写。
    let hir = compile(LOCAL_SHADOW_SOURCE);
    let run = function(&hir, "run");
    let (_, argument) = outer_call(run);
    assert!(matches!(argument.kind, HirExprKind::Variable(_)), "局部绑定优先于自由函数名: {:?}", argument.kind);
}

#[test]
fn instantiation_is_deterministic_across_compilations() {
    let first = compile(GENERIC_CONSUMER_SOURCE);
    let second = compile(GENERIC_CONSUMER_SOURCE);
    let projected = |module: &rusty_valkyrie::types::hir::HirModule| {
        module
            .instantiated_callables
            .iter()
            .map(|callable| (callable.function.name.clone(), callable.function.instance))
            .collect::<Vec<_>>()
    };
    assert_eq!(projected(&first), projected(&second), "同一输入的实例空间必须稳定");
    let (_, first_argument) = outer_call(function(&first, "run"));
    let (_, second_argument) = outer_call(function(&second, "run"));
    assert_eq!(reference(first_argument).instance, reference(second_argument).instance);
}

#[test]
fn cross_module_free_function_reference_is_not_rewritten() {
    // 非当前模块的裸名不满足无限定自由调用对齐：保持现状，沿既有失败路径。
    const LIBRARY: &str = r#"
namespace lib;

micro cmp_i64(a: i64, b: i64) -> i32 {
    return 0
}
"#;
    const CONSUMER: &str = r#"
namespace app;

micro apply(a: i64, b: i64, cmp: micro(i64, i64) -> i32) -> i32 {
    return cmp(a, b)
}

[export] micro run() -> i32 {
    return apply(3, 5, cmp_i64)
}
"#;
    let groups = [
        CompilerSourceGroup::single_source("lib", "lib", LIBRARY),
        CompilerSourceGroup::single_source("app", "app", CONSUMER).as_entry().with_direct_dependencies(vec!["lib".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH);
    match modules {
        Ok(modules) => {
            let run = modules.iter().flat_map(|module| module.functions.iter()).find(|item| item.name.as_str() == "run").expect("run");
            let (_, argument) = outer_call(run);
            assert!(matches!(argument.kind, HirExprKind::Path(_)), "跨模块裸名不改写: {:?}", argument.kind);
        }
        Err(error) => assert!(matches!(error, HirError::UnresolvedCall { .. }), "跨模块裸名必须沿既有路径失败: {error:?}"),
    }
}
