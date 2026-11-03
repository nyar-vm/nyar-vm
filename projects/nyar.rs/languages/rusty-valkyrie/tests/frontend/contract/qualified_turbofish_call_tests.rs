//! 限定路径 + 显式泛型调用的 frontend 守卫：`a::b::f::<T>(x)` 必须解析成
//! `GenericApply` 调用，不得被拆成 `<` / `>` 比较链。
//!
//! 解析器在路径段循环里遇到 `::<`（turbofish）时必须停在 `::` 之前；否则
//! `std::collection::array::new::<T>(old_len + 1)` 会变成 `a::b` 与
//! `<T>(...)` 的比较，前端报 `UnresolvedCall { selector: "infix >" }`。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    compiler::types::{
        Identifier, NamePath,
        hir::{HirExprKind, HirFunction, HirModule},
    },
};

/// 数值原始类型名属于 `std/primitive` 的声明族；`usize` 是生产 std 的指针宽度别名。
const PRIMITIVES: &str = "[primitive(\"std::primitive::u64\")] structure u64 { } \
    type usize = u64; \
    [primitive(\"std::primitive::i32\")] structure i32 { } ";

fn compile_named(source: &str, module: &str) -> Result<HirModule, HirError> {
    let name = NamePath::new(module.split('.').map(Identifier::new).collect::<Vec<_>>());
    ValkyrieCompiler::default().compile_source_with_semantic_exports_and_name(source, &[], Some(name))
}

fn find_function<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 `{name}` 声明"))
}

/// 尾部 `return` 是块尾表达式（`body.expr`），不是普通语句。
fn probe_tail_expression(function: &HirFunction) -> &rusty_valkyrie::compiler::types::hir::HirExpr {
    let expression = function.body.expr.as_deref().unwrap_or_else(|| panic!("expected trailing return expression: {:?}", function.body));
    let HirExprKind::Return(Some(expression)) = &expression.kind
    else {
        panic!("expected return expression: {expression:?}");
    };
    expression.as_ref()
}

/// 三段限定路径 + `::<T>` 必须作为一次泛型调用绑定到声明，而不是比较链。
#[test]
fn qualified_turbofish_call_binds_declaration() {
    let source = format!(
        "{PRIMITIVES}\
         micro add_one<T>(value: T): T {{ return value }} \
         micro probe_value(): bool {{ return probe::nested::add_one::<bool>(true) }}"
    );
    let hir = compile_named(&source, "probe.nested").expect("限定路径 + turbofish 调用必须完成 HIR");
    let expression = probe_tail_expression(find_function(&hir, "probe_value"));
    let HirExprKind::Call { resolved, .. } = &expression.kind
    else {
        panic!("限定路径 + turbofish 必须落在 Call，不得只剩比较链: {expression:?}");
    };
    let declaration = find_function(&hir, "add_one").declaration;
    assert!(resolved.as_ref().is_some_and(|resolved| resolved.declaration == declaration), "调用必须绑定 `add_one` 声明身份: {resolved:?}");
}

/// 运算符没有声明候选时仍是 `UnresolvedCall`，不得被任何名字表兜底。
#[test]
fn undeclared_operator_operand_type_fails_closed() {
    let source = format!("{PRIMITIVES}micro compare(): bool {{ return 1 > 2 }}");
    let error = ValkyrieCompiler::default().compile_source(&source).expect_err("未声明的 `infix >` 必须失败闭合");
    match error {
        HirError::UnresolvedCall { selector, .. } => assert_eq!(selector, "infix >", "失败必须落在缺少声明的运算符成员"),
        other => panic!("期望 UnresolvedCall，实际 {other:?}"),
    }
}

/// 构造位置的 turbofish（`Wrap::<T> { .. }`）必须保留名字路径并解析构造，
/// 不得退化成占位 `_` 路径。
#[test]
fn turbofish_constructor_binds_declaration() {
    let source = format!(
        "{PRIMITIVES}\
         structure Wrap<T> {{ _value: T, _index: usize }} \
         micro make<T>(value: T): Wrap<T> {{ return Wrap::<T> {{ _value: value, _index: 0 }} }}"
    );
    let hir = compile_named(&source, "probe.nested").expect("构造位置 turbofish 必须完成 HIR");
    let expression = probe_tail_expression(find_function(&hir, "make"));
    let HirExprKind::Construct { path, generic_arguments, resolved, .. } = &expression.kind
    else {
        panic!("构造位置 turbofish 必须落在 Construct: {expression:?}");
    };
    assert_eq!(path.to_string(), "Wrap", "turbofish 构造必须保留构造名而不是 `_` 占位");
    assert_eq!(generic_arguments.len(), 1, "`::<T>` 必须作为显式泛型实参保留: {generic_arguments:?}");
    assert!(resolved.is_some(), "构造必须解析到候选: {resolved:?}");
}

/// 显式泛型实参数量与 binder 不符时必须失败闭合，不得静默丢弃 turbofish。
#[test]
fn turbofish_constructor_wrong_arity_fails_closed() {
    let source = format!(
        "{PRIMITIVES}\
         structure Wrap<T> {{ _value: T, _index: usize }} \
         micro make<T>(value: T): Wrap<T> {{ return Wrap::<T, i32> {{ _value: value, _index: 0 }} }}"
    );
    let error = compile_named(&source, "probe.nested").expect_err("binder 数量不符的 turbofish 构造必须失败闭合");
    match error {
        HirError::UnresolvedConstructor { name, .. } => assert_eq!(name, "Wrap", "失败必须落在构造名"),
        other => panic!("期望 UnresolvedConstructor，实际 {other:?}"),
    }
}
