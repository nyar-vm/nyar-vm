//! `array.new` 的 frontend 守卫：前端不再从任何注解规范化出 `ArrayNew`。
//!
//! `[intrinsic]` 注解已退役、对前端惰性；无注解普通声明函数调用必须保持
//! `HirExprKind::Call` 并绑定声明身份，不得按名字或形态特判；
//! 声明存在但实参 arity 不符时必须以 `UnresolvedCall` 显式失败。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    compiler::types::hir::{HirExpr, HirExprKind, HirFunction, HirModule},
};

/// 数值原始类型名属于 `std/primitive` 的声明族；`usize` 是生产 std 的指针宽度别名。
const PRIMITIVES: &str = "[primitive(\"std::primitive::u64\")] structure u64 { } \
    type usize = u64; \
    [primitive(\"std::primitive::i32\")] structure i32 { } ";

fn compile(source: &str) -> HirModule {
    ValkyrieCompiler::default().compile_source(source).expect("源码必须完成 HIR")
}

fn find_function<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 `{name}` 声明"))
}

/// 尾部 `return` 是块尾表达式（`body.expr`），不是普通语句。
fn probe_tail_expression(function: &HirFunction) -> &HirExpr {
    let expression = function.body.expr.as_deref().unwrap_or_else(|| panic!("expected trailing return expression: {:?}", function.body));
    let HirExprKind::Return(Some(expression)) = &expression.kind
    else {
        panic!("expected return expression: {expression:?}");
    };
    expression.as_ref()
}

#[test]
fn ordinary_declared_function_call_stays_hir_call_bound_to_declaration() {
    let source = format!(
        "{PRIMITIVES}\
         micro alloc(length: usize): [i32] {{ }} \
         micro probe() -> [i32] {{ return alloc(4) }}"
    );
    let hir = compile(&source);
    let expression = probe_tail_expression(find_function(&hir, "probe"));
    let HirExprKind::Call { resolved, .. } = &expression.kind
    else {
        panic!("普通声明函数调用必须保持 Call，不得被规范化成任何结构节点, got {expression:?}");
    };
    let alloc_declaration = find_function(&hir, "alloc").declaration;
    assert!(
        resolved.as_ref().is_some_and(|resolved| resolved.declaration == alloc_declaration),
        "调用必须绑定源码声明的完整身份: {resolved:?}"
    );
}

#[test]
fn declared_function_with_wrong_arity_fails_as_unresolved_call() {
    let source = format!(
        "{PRIMITIVES}\
         micro alloc<T>(length: usize, extra: usize): [T] {{ }} \
         micro probe() -> [i32] {{ return alloc::<i32>(4) }}"
    );
    let error = ValkyrieCompiler::default().compile_source(&source).expect_err("wrong arity must fail closed");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "arity 不符必须失败在调用边界为 UnresolvedCall: {error:?}");
}
