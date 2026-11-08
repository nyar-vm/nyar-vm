//! 数组内建的前端身份合同：`length` / `new` / `push` 只来自源码声明，注解退役后不得合成候选。
//!
//! 三处合同共用同一套「声明存在/缺失」对拍：
//! 1. 裸 `[T]` 的 `length(values)` 无声明时必须以 `HirError::UnresolvedCall` 显式失败；
//! 2. 普通声明的 `alloc` 必须保持 `HirExprKind::Call` 并绑定声明身份，实参 arity 不符时 `UnresolvedCall`；
//! 3. `push` 的声明是唯一前端来源，删除声明后调用（含限定名形态）必须结构化失败，不得靠名字表兜底。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    compiler::types::hir::{HirExpr, HirExprKind, HirFunction, HirModule},
    types::hir::HirStatementKind,
};

/// 与 std 声明同形的唯一前端来源；普通声明 callable，不携带任何退役注解。
const ARRAY_PUSH_DECLARATION: &str = "micro push<T>(array: [T], value: T) -> [T] { } ";

const GROW: &str = "micro grow(text: utf8) -> [utf8] { let mut result: [utf8] = [] push(result, text) return result }";

const QUALIFIED_GROW: &str = "micro grow(text: utf8) -> [utf8] { let mut result: [utf8] = [] builtin.array.push(result, text) return result }";

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

fn push_call(function: &HirFunction) -> &rusty_valkyrie::compiler::types::hir::HirResolvedCall {
    for statement in &function.body.statements {
        if let HirStatementKind::Expr(expression) = &statement.kind
            && let HirExprKind::Call { resolved: Some(resolved), .. } = &expression.kind
        {
            return resolved;
        }
    }
    panic!("expected a resolved push call: {:?}", function.body);
}

#[test]
fn undeclared_array_length_call_fails_without_frontend_synthesis() {
    let error = ValkyrieCompiler::default()
        .compile_source_to_program(&crate::primitives::with_primitives("micro probe(values: [i32]) -> usize { return length(values) }"))
        .expect_err("缺少声明时数组长度调用不得成功");
    let rendered = format!("{error:?}");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "缺少声明必须结构化失败为 UnresolvedCall: {rendered}");
    assert!(rendered.contains("length"), "失败必须指出未解析的 length 调用: {rendered}");
}

#[test]
fn ordinary_declared_function_call_stays_hir_call_bound_to_declaration() {
    let source = crate::primitives::with_primitives(
        "micro alloc(length: usize): [i32] { } \
         micro probe() -> [i32] { return alloc(4) }",
    );
    let hir = ValkyrieCompiler::default().compile_source(&source).expect("源码必须完成 HIR");
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
    let source = crate::primitives::with_primitives(
        "micro alloc<T>(length: usize, extra: usize): [T] { } \
         micro probe() -> [i32] { return alloc::<i32>(4) }",
    );
    let error = ValkyrieCompiler::default().compile_source(&source).expect_err("wrong arity must fail closed");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "arity 不符必须失败在调用边界为 UnresolvedCall: {error:?}");
}

#[test]
fn declared_array_push_binds_declared_callable_identity() {
    let compiler = ValkyrieCompiler::default();
    let hir = compiler
        .compile_source(&format!("{ARRAY_PUSH_DECLARATION}{GROW}"))
        .expect("源码内声明的数组 push 必须解析");

    let declaration = find_function(&hir, "push").declaration;
    let resolved = push_call(find_function(&hir, "grow"));
    assert_eq!(resolved.declaration, declaration, "调用必须绑定到源码声明的完整身份，而不是前端合成候选");
    assert_eq!(resolved.symbol.parts().last().map(|part| part.as_str()), Some("push"), "符号必须是声明名，而不是合成路径: {}", resolved.symbol);
}

#[test]
fn undeclared_array_push_call_fails_without_frontend_synthesis() {
    for probe in [GROW, QUALIFIED_GROW] {
        let error = ValkyrieCompiler::default()
            .compile_source_to_program(&crate::primitives::with_primitives(probe))
            .expect_err("缺少声明时数组 push 调用不得成功");
        let rendered = format!("{error:?}");
        assert!(matches!(error, HirError::UnresolvedCall { .. }), "缺少声明必须结构化失败为 UnresolvedCall: {rendered}");
        assert!(rendered.contains("push"), "失败必须指出未解析的 push 调用: {rendered}");
    }
}
