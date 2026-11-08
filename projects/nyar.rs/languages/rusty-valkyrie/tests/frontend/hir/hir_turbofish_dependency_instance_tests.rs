//! 显式泛型实参（turbofish）调用的实例化合同。
//!
//! 显式实参与隐式推断只是 substitution 的来源不同：两者都必须在调用解析层绑定
//! 到同一份声明身份，并由单态化固定点铸造同一个 `ItemInstanceId`。跨源码组时
//! consumer 不需要 `using`，按依赖语义导出的声明身份命中即可。

use rusty_valkyrie::{
    CompilerSourceGroup, HirError, ValkyrieCompiler,
    compiler::types::hir::{HirBlock, HirExpr, HirExprKind, HirFunction, HirModule, HirResolvedCall, HirStatementKind},
    hir::lowering::DEFAULT_COMPILE_ARCH,
};

use crate::primitives::SourceFixtureCompile;

fn compile_source(source: &str) -> HirModule {
    ValkyrieCompiler::default().compile_fixture_source(source).expect("turbofish 源码必须完成 HIR")
}

fn compile_groups(groups: &[CompilerSourceGroup]) -> Vec<HirModule> {
    let groups = crate::primitives::groups_with_primitives(groups.to_vec());
    ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).expect("跨组 turbofish 调用必须完成 HIR")
}

fn find_function<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 `{name}` 声明"))
}

/// 深遍历函数体全部已解析调用，覆盖尾块表达式与嵌套调用。
fn body_calls(function: &HirFunction) -> Vec<&HirResolvedCall> {
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
    fn walk_expr<'a>(expr: &'a HirExpr, calls: &mut Vec<&'a HirResolvedCall>) {
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
            HirExprKind::Return(Some(inner)) | HirExprKind::Yield(Some(inner)) | HirExprKind::FieldInit { value: inner, .. } => {
                walk_expr(inner, calls)
            }
            HirExprKind::Block(block) => walk_block(block, calls),
            HirExprKind::Construct { args, .. } => {
                for arg in args {
                    walk_expr(arg, calls);
                }
            }
            _ => {}
        }
    }
    let mut calls = Vec::new();
    walk_block(&function.body, &mut calls);
    calls
}

/// 依赖组：`Option` 的零字段变体与泛型自由构造函数在同一个源码组。
const LIBRARY: &str = r#"
namespace core::types;

unite Option<T> {
    Some { value: T },
    [tag(1, default)]
    None,
}

micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}

micro wrap<T>(value: T) -> T {
    return value
}
"#;

/// consumer：不带 `using`，显式与隐式两条路径都走依赖导出的同一份声明。
const CONSUMER: &str = r#"
namespace app;

[export] micro run(): i64 {
    let optional: Option<i64> = option_none::<i64>()
    let seed: i64 = 1
    let explicit: i64 = wrap::<i64>(seed)
    let inferred: i64 = wrap(seed)
    return explicit
}
"#;

fn cross_group_modules() -> Vec<HirModule> {
    let groups = [
        CompilerSourceGroup::single_source("std", "std", LIBRARY),
        CompilerSourceGroup::single_source("app", "app", CONSUMER).as_entry().with_direct_dependencies(vec!["std".into()]),
    ];
    compile_groups(&groups)
}

/// 跨组守护：consumer 不带 `using` 时，显式 turbofish 与隐式推断必须落到同一实例身份。
#[test]
fn cross_group_explicit_and_inferred_calls_share_one_instance() {
    let modules = cross_group_modules();
    let app = modules.iter().find(|module| module.functions.iter().any(|function| function.name.as_str() == "run")).expect("app 模块");
    let wrap_declaration = app
        .functions
        .iter()
        .find(|function| function.name.as_str() == "wrap")
        .and_then(|function| function.declaration)
        .or_else(|| {
            modules
                .iter()
                .find_map(|module| module.functions.iter().find(|function| function.name.as_str() == "wrap"))
                .and_then(|function| function.declaration)
        })
        .expect("`wrap` 声明身份");
    let calls = body_calls(find_function(app, "run"));
    let wrap_calls = calls.iter().filter(|call| call.declaration == Some(wrap_declaration)).collect::<Vec<_>>();
    assert_eq!(wrap_calls.len(), 2, "显式与隐式两处调用都必须绑定 `wrap` 声明: {wrap_calls:?}");
    let instances = wrap_calls.iter().map(|call| call.instance).collect::<Vec<_>>();
    assert!(instances[0].is_some(), "显式 turbofish 必须铸造实例: {instances:?}");
    assert_eq!(instances[0], instances[1], "显式实参与隐式推断必须产生同一 `ItemInstanceId`: {instances:?}");
}

/// 零字段可选值的显式实例化：`Option` 的空变体不参与类型推断，实例只由
/// turbofish 的 substitution 决定；解析必须给出已铸造的实例身份。
#[test]
fn zero_field_optional_explicit_instantiation_resolves_to_instance() {
    let module = compile_source(
        r#"
namespace probe;

unite Option<T> {
    Some { value: T },
    [tag(1, default)]
    None,
}

micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}

[export] micro run(): Option<i64> {
    return option_none::<i64>()
}
"#,
    );
    let calls = body_calls(find_function(&module, "run"));
    let call = calls.iter().find(|call| call.declaration.is_some()).expect("必须存在已解析调用");
    assert!(call.instance.is_some(), "显式实例化必须由单态化固定点铸造实例: {call:?}");
}

/// 带类型参数的嵌套调用：外层实例体内的 `inner::<T>` 在代入后必须落在具体实例上。
#[test]
fn nested_generic_call_inside_instance_resolves_to_instance() {
    let module = compile_source(
        r#"
namespace probe;

micro inner<T>(value: T) -> T {
    return value
}

micro outer<T>(value: T) -> T {
    return inner::<T>(value)
}

[export] micro run(): i64 {
    let seed: i64 = 1
    return outer::<i64>(seed)
}
"#,
    );
    let outer_declaration = find_function(&module, "outer").declaration.expect("outer 声明身份");
    let inner_declaration = find_function(&module, "inner").declaration.expect("inner 声明身份");
    let instance = module
        .instantiated_callables
        .iter()
        .find(|callable| callable.function.declaration == Some(outer_declaration))
        .expect("outer<i64> 实例必须由固定点铸造");
    let calls = body_calls(&instance.function);
    let inner_call = calls.iter().find(|call| call.declaration == Some(inner_declaration)).expect("实例体内必须保留 `inner` 调用");
    assert!(inner_call.instance.is_some(), "嵌套调用的显式实参必须解析成实例: {inner_call:?}");
}

/// 显式 + 隐式混用：同一函数的两次调用必须选中同一实例，不得按调用点各铸一份。
#[test]
fn explicit_and_inferred_calls_share_one_instance_in_group() {
    let module = compile_source(
        r#"
namespace probe;

micro wrap<T>(value: T) -> T {
    return value
}

[export] micro run(): i64 {
    let seed: i64 = 1
    let explicit: i64 = wrap::<i64>(seed)
    let inferred: i64 = wrap(seed)
    return explicit
}
"#,
    );
    let declaration = find_function(&module, "wrap").declaration.expect("wrap 声明身份");
    let calls = body_calls(find_function(&module, "run"));
    let instances = calls.iter().filter(|call| call.declaration == Some(declaration)).map(|call| call.instance).collect::<Vec<_>>();
    assert_eq!(instances.len(), 2, "两处 `wrap` 调用都必须解析: {instances:?}");
    assert_eq!(instances[0], instances[1], "显式与隐式必须共享同一实例身份: {instances:?}");
    assert!(instances[0].is_some(), "实例身份必须由固定点铸造: {instances:?}");
}

/// 错误实参数量负例：名称仍在作用域，但显式实参数量与 binder 不一致时必须是结构化
/// 诊断，直接给出期望 binder 数量/种类与实际实参。
#[test]
fn explicit_generic_arity_mismatch_reports_expected_and_actual() {
    let error = ValkyrieCompiler::default()
        .compile_source(&crate::primitives::with_primitives(
            r#"
namespace probe;

unite Option<T> {
    Some { value: T },
    [tag(1, default)]
    None,
}

micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}

[export] micro run(): Option<i64> {
    return option_none::<i64, bool>()
}
"#,
        ))
        .expect_err("显式实参数量不符必须失败闭合");
    let HirError::UnresolvedCall { selector, reason, .. } = &error
    else {
        panic!("必须是未解析调用诊断: {error}");
    };
    assert_eq!(selector, "option_none::<i64, bool>", "callee 必须保留 peel 后的名字与显式实参");
    assert_eq!(reason.tag(), "explicit-generic-arguments-mismatch");
    let message = format!("{error}");
    assert!(message.contains("expected 1 binder(s) (type)"), "诊断必须给出期望 binder 数量/种类: {message}");
    assert!(message.contains("got 2 explicit argument(s)"), "诊断必须给出实际实参数量: {message}");
    assert!(message.contains("i64, bool"), "诊断必须列出实际实参: {message}");
}
