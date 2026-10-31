//! `builtin.array.new` 的 frontend 规范化合同。
//!
//! 源码声明 `[intrinsic("builtin.array.new")]` 是唯一身份来源；调用解析之后由
//! post-binding pass 依据冻结的返回数组形状规范化为 `HirExprKind::ArrayNew`。
//! 无注解的普通函数调用必须保持普通 `Call`，不得按名字或形态特判。

use rusty_valkyrie::{
    HirError, MirLowerer, MirOperation, ValkyrieCompiler,
    compiler::types::hir::{HirExpr, HirExprKind, HirFunction, HirModule, ValkyrieType},
};

/// 数值原始类型名属于 `core/primitive` 的声明族；未声明时 canonical 以 CAN003 拒绝。
const PRIMITIVES: &str = "[primitive(\"core::primitive::usize\")] structure usize { } \
    [primitive(\"core::primitive::i32\")] structure i32 { } ";

/// 唯一的 `builtin.array.new` 身份来源。
const ARRAY_NEW_DECLARATION: &str = "[intrinsic(\"builtin.array.new\")] private micro alloc<T>(length: usize): [T] { } ";

/// 具体元素类型由显式泛型实参冻结：`alloc::<i32>(4)`。
const PROBE: &str = "micro probe() -> [i32] { return alloc::<i32>(4) }";

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

fn declared_type_declaration(module: &HirModule, name: &str) -> nyar_types::ItemId {
    module
        .structs
        .iter()
        .find(|item| item.name.as_str() == name)
        .and_then(|item| item.constructor_declaration)
        .unwrap_or_else(|| panic!("缺少 `{name}` 声明身份"))
}

#[test]
fn declared_array_new_intrinsic_normalizes_to_hir_array_new() {
    let source = format!("{PRIMITIVES}{ARRAY_NEW_DECLARATION}{PROBE}");
    let hir = compile(&source);

    let expression = probe_tail_expression(find_function(&hir, "probe"));
    let HirExprKind::ArrayNew { element_type, length } = &expression.kind
    else {
        panic!("declared `builtin.array.new` call must normalize to ArrayNew, got {expression:?}");
    };

    let expected = ValkyrieType::Nominal { declaration: declared_type_declaration(&hir, "i32"), arguments: Vec::new() };
    assert_eq!(element_type, &expected, "element type must come from the resolved return array shape");
    assert!(
        matches!(length.kind, HirExprKind::Literal(rusty_valkyrie::types::hir::HirLiteral::Integer64(_))),
        "length must stay the original expression: {length:?}"
    );

    let call_text = "alloc::<i32>(4)";
    let call_start = source.find(call_text).expect("call text");
    assert_eq!(expression.span.get_start() as usize, call_start, "ArrayNew must preserve the call span start");
    assert_eq!(expression.span.get_end() as usize, call_start + call_text.len(), "ArrayNew must preserve the call span end");
}

#[test]
fn array_new_normalization_descends_into_nested_expressions() {
    let source = format!(
        "{PRIMITIVES}{ARRAY_NEW_DECLARATION}\
         micro probe(flag: bool) -> [i32] {{ if flag {{ return alloc::<i32>(4) }} return alloc::<i32>(5) }}"
    );
    let hir = compile(&source);
    let probe = find_function(&hir, "probe");

    fn collect(expr: &HirExpr, out: &mut Vec<ValkyrieType>) {
        match &expr.kind {
            HirExprKind::ArrayNew { element_type, length } => {
                out.push(element_type.clone());
                collect(length, out);
            }
            HirExprKind::Return(Some(inner)) => collect(inner, out),
            HirExprKind::If { condition, then_branch, else_branch } => {
                collect(condition, out);
                if let Some(expr) = then_branch.expr.as_deref() {
                    collect(expr, out);
                }
                for statement in &then_branch.statements {
                    if let rusty_valkyrie::compiler::types::hir::HirStatementKind::Expr(expr) = &statement.kind {
                        collect(expr, out);
                    }
                }
                if let Some(branch) = else_branch {
                    if let Some(expr) = branch.expr.as_deref() {
                        collect(expr, out);
                    }
                    for statement in &branch.statements {
                        if let rusty_valkyrie::compiler::types::hir::HirStatementKind::Expr(expr) = &statement.kind {
                            collect(expr, out);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut elements = Vec::new();
    for statement in &probe.body.statements {
        if let rusty_valkyrie::compiler::types::hir::HirStatementKind::Expr(expr) = &statement.kind {
            collect(expr, &mut elements);
        }
    }
    if let Some(expr) = probe.body.expr.as_deref() {
        collect(expr, &mut elements);
    }
    assert_eq!(elements.len(), 2, "both nested calls must normalize: {elements:?}");
    let expected = ValkyrieType::Nominal { declaration: declared_type_declaration(&hir, "i32"), arguments: Vec::new() };
    assert!(elements.iter().all(|element| element == &expected), "nested element types must stay concrete: {elements:?}");
}

#[test]
fn concrete_array_new_lowers_to_mir_array_new() {
    let source = format!(
        "{PRIMITIVES}{ARRAY_NEW_DECLARATION}\
         micro probe() -> [i32] {{ let values: [i32] = alloc::<i32>(4) return values }}"
    );
    let hir = compile(&source);
    let semantic = MirLowerer::lower_module_semantic(&hir);
    assert!(semantic.diagnostics.is_empty(), "concrete ArrayNew must lower without diagnostics: {:?}", semantic.diagnostics);

    let mut observed = None;
    for function in &semantic.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let MirOperation::ArrayNew { array_type, .. } = &instruction.kind {
                    observed = Some(array_type.clone());
                }
            }
        }
    }
    let array_type = observed.expect("MIR must produce ArrayNew for a concrete element type");
    let ValkyrieType::Array(element) = &array_type
    else {
        panic!("ArrayNew array_type must be an array shape, got {array_type:?}");
    };
    let expected = ValkyrieType::Nominal { declaration: declared_type_declaration(&hir, "i32"), arguments: Vec::new() };
    assert_eq!(element.as_ref(), &expected, "ArrayNew must carry the frozen concrete element type");
    assert!(
        !matches!(element.as_ref(), ValkyrieType::Named(name) if name.as_str() == "i32"),
        "element type must be Nominal identity, not a surface name"
    );
}

#[test]
fn generic_array_new_element_type_is_substituted_by_monomorphization() {
    let source = format!(
        "{PRIMITIVES}{ARRAY_NEW_DECLARATION}\
         micro make<T>(length: usize) -> [T] {{ return alloc::<T>(length) }} \
         micro caller() -> [i32] {{ return make::<i32>(3) }}"
    );
    let hir = compile(&source);
    let expected = ValkyrieType::Nominal { declaration: declared_type_declaration(&hir, "i32"), arguments: Vec::new() };

    let mut observed = false;
    for callable in &hir.instantiated_callables {
        if callable.function.name.as_str() != "make" {
            continue;
        }
        let expression = probe_tail_expression(&callable.function);
        let HirExprKind::ArrayNew { element_type, .. } = &expression.kind
        else {
            panic!("instantiated `make` body must keep ArrayNew, got {expression:?}");
        };
        assert_eq!(element_type, &expected, "monomorphization must replace the generic binder with the concrete element type");
        assert!(!format!("{element_type:?}").contains("binder: Some("), "instantiated element type must be closed: {element_type:?}");
        observed = true;
    }
    assert!(observed, "caller must instantiate `make<i32>`");
}

#[test]
fn ordinary_function_without_annotation_is_never_treated_as_array_new() {
    let source = format!(
        "{PRIMITIVES}\
         micro alloc(length: usize): [i32] {{ }} \
         micro probe() -> [i32] {{ return alloc(4) }}"
    );
    let hir = compile(&source);
    let expression = probe_tail_expression(find_function(&hir, "probe"));
    let HirExprKind::Call { resolved, .. } = &expression.kind
    else {
        panic!("without the intrinsic annotation the call must stay a Call, got {expression:?}");
    };
    assert!(resolved.as_ref().is_some_and(|resolved| resolved.intrinsic.is_none()), "普通函数调用不得获得内建结构身份: {resolved:?}");
}

#[test]
fn array_new_intrinsic_with_non_array_return_fails_closed() {
    let source = format!(
        "{PRIMITIVES}\
         [intrinsic(\"builtin.array.new\")] private micro alloc<T>(length: usize): usize {{ }} \
         micro probe() -> usize {{ return alloc::<i32>(4) }}"
    );
    let error = ValkyrieCompiler::default().compile_source(&source).expect_err("non-array return must fail closed");
    assert!(matches!(error, HirError::ArrayNewReturnTypeNotArray { .. }), "non-array return must fail with the structured variant: {error:?}");
}

#[test]
fn array_new_intrinsic_with_wrong_arity_fails_closed() {
    let source = format!(
        "{PRIMITIVES}\
         [intrinsic(\"builtin.array.new\")] private micro alloc<T>(length: usize, extra: usize): [T] {{ }} \
         micro probe() -> [i32] {{ return alloc::<i32>(4) }}"
    );
    let error = ValkyrieCompiler::default().compile_source(&source).expect_err("wrong arity must fail closed");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "missing length contract must fail at the call boundary: {error:?}");
}
