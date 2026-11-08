//! 块 3：`[constructor]` 括号调用门禁，以及字面量构造入口的保留与交叉使用。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    types::hir::{HirBlock, HirCallableDomain, HirExpr, HirExprKind, HirModule, HirResolvedCall, HirStatementKind, ValkyrieType},
};

use crate::primitives::SourceFixtureCompile;

fn compile(source: &str) -> HirModule {
    ValkyrieCompiler::default().compile_fixture_source(source).unwrap_or_else(|error| panic!("源码必须完成 HIR lowering: {error}"))
}

fn compile_error(source: &str) -> HirError {
    ValkyrieCompiler::default().compile_fixture_source(source).expect_err("该源码必须在 HIR 合同边界结构化失败")
}

fn resolved_calls_in_function<'a>(module: &'a HirModule, name: &str) -> Vec<&'a HirResolvedCall> {
    fn walk_block<'a>(block: &'a HirBlock, out: &mut Vec<&'a HirResolvedCall>) {
        for statement in &block.statements {
            match &statement.kind {
                HirStatementKind::Let { initializer: Some(expr), .. } => walk_expr(expr, out),
                HirStatementKind::Expr(expr) => walk_expr(expr, out),
                _ => {}
            }
        }
        if let Some(expr) = block.expr.as_deref() {
            walk_expr(expr, out);
        }
    }

    fn walk_expr<'a>(expr: &'a HirExpr, out: &mut Vec<&'a HirResolvedCall>) {
        match &expr.kind {
            HirExprKind::Call { callee, args, resolved } => {
                if let Some(resolved) = resolved {
                    out.push(resolved);
                }
                walk_expr(callee, out);
                for arg in args {
                    walk_expr(&arg.value, out);
                }
            }
            HirExprKind::Construct { args, resolved, .. } => {
                if let Some(resolved) = resolved {
                    out.push(resolved);
                }
                for arg in args {
                    walk_expr(arg, out);
                }
            }
            HirExprKind::Return(Some(inner)) => walk_expr(inner, out),
            HirExprKind::Match { scrutinee, arms } | HirExprKind::Case { scrutinee, arms } => {
                walk_expr(scrutinee, out);
                for arm in arms {
                    walk_expr(&arm.body, out);
                    if let Some(guard) = &arm.guard {
                        walk_expr(guard, out);
                    }
                }
            }
            HirExprKind::Block(block) => walk_block(block, out),
            HirExprKind::If { condition, then_branch, else_branch } => {
                walk_expr(condition, out);
                walk_block(then_branch, out);
                if let Some(else_branch) = else_branch {
                    walk_block(else_branch, out);
                }
            }
            HirExprKind::TupleLiteral { items } | HirExprKind::ArrayLiteral { items } => {
                for item in items {
                    walk_expr(item, out);
                }
            }
            _ => {}
        }
    }

    let function = module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 {name}"));
    let mut calls = Vec::new();
    walk_block(&function.body, &mut calls);
    calls
}

fn assert_option_of_integer32(function: &str, call: &HirResolvedCall, option_declaration: nyar_types::ItemId) {
    assert_eq!(call.domain, HirCallableDomain::Constructor, "{function}: 必须命中 Constructor 域");
    assert!(call.sum_owner.is_some() && call.sum_variant.is_some(), "{function}: 构造结果必须携带 sum_owner/sum_variant");
    assert_eq!(call.parameter_types, vec![ValkyrieType::Integer32 { signed: true }], "{function}: 单字段参数类型必须来自字段声明");
    match &call.return_type {
        ValkyrieType::Nominal { declaration, arguments, .. } => {
            assert_eq!(*declaration, option_declaration, "{function}: 返回类型必须指向 unite 声明身份");
            assert_eq!(
                arguments,
                &vec![ValkyrieType::Integer32 { signed: true }],
                "{function}: `Some(1)` 必须经既有泛型实例化推出 `Option<i32>`"
            );
        }
        other => panic!("{function}: 返回类型必须是 Nominal，got {other:?}"),
    }
}

const MARKED_OPTION: &str = r#"
unite Option<T> {
    [constructor]
    Some {
        value: T,
    },
    None,
}
"#;

fn option_declaration(module: &HirModule) -> nyar_types::ItemId {
    module.enums.iter().find(|item| item.name.as_str() == "Option").and_then(|item| item.declaration).expect("Option declaration")
}

#[test]
fn marked_variant_bracket_call_resolves_through_constructor_domain() {
    let module = compile(&format!("{MARKED_OPTION}\nmicro produce() {{ let present = Some(1) }}\n"));
    let calls = resolved_calls_in_function(&module, "produce");
    assert_eq!(calls.len(), 1, "`Some(1)` 必须产生一次已解析调用");
    let call = calls[0];
    assert_option_of_integer32("produce", call, option_declaration(&module));
    assert_eq!(call.symbol.parts().last().map(|part| part.as_str()), Some("Some"));
}

#[test]
fn unmarked_variant_bracket_call_fails_closed() {
    let error = compile_error(
        r#"
unite Plain {
    Present {
        value: i64,
    },
    Absent,
}
micro produce() {
    let present = Present(1)
}
"#,
    );
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "未标注 [constructor] 的变体不得经括号调用构造: {error:?}");
}

#[test]
fn bracket_call_argument_count_mismatch_fails_closed() {
    let error = compile_error(
        r#"
unite Pair {
    [constructor]
    Both {
        left: bool,
        right: bool,
    },
    Neither,
}
micro produce() {
    let present = Both(true)
}
"#,
    );
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "参数个数与字段数不符必须结构化失败: {error:?}");
}

#[test]
fn bracket_call_argument_type_mismatch_fails_closed() {
    let error = compile_error(
        r#"
unite Wrapped {
    [constructor]
    Flag {
        on: bool,
    },
    Off,
}
micro produce() {
    let present = Flag(1)
}
"#,
    );
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "实参类型与字段类型不符必须结构化失败: {error:?}");
}

#[test]
fn literal_construction_stays_available_with_constructor_attribute() {
    let module = compile(&format!("{MARKED_OPTION}\nmicro produce() {{ let literal = Some {{ value: 1 }} }}\n"));
    let calls = resolved_calls_in_function(&module, "produce");
    assert_eq!(calls.len(), 1, "`Some {{ value: 1 }}` 必须产生一次已解析构造");
    let call = calls[0];
    assert_option_of_integer32("produce", call, option_declaration(&module));
    assert_eq!(call.symbol.parts().last().map(|part| part.as_str()), Some("Some"));
}

#[test]
fn literal_construction_stays_available_without_constructor_attribute() {
    let module = compile(
        r#"
unite Plain<T> {
    Present {
        value: T,
    },
    Absent,
}
micro produce() {
    let literal = Present { value: 1 }
}
"#,
    );
    let calls = resolved_calls_in_function(&module, "produce");
    assert_eq!(calls.len(), 1, "无 [constructor] 的变体也必须保留字面量构造");
    let call = calls[0];
    assert_eq!(call.domain, HirCallableDomain::Constructor);
    assert!(call.sum_owner.is_some() && call.sum_variant.is_some(), "字面量构造的产物身份必须与变体声明一致");
    assert_eq!(call.parameter_types, vec![ValkyrieType::Integer32 { signed: true }]);
    match &call.return_type {
        ValkyrieType::Nominal { arguments, .. } => {
            assert_eq!(arguments, &vec![ValkyrieType::Integer32 { signed: true }]);
        }
        other => panic!("`Present {{ value: 1 }}` 返回类型必须是 Nominal，got {other:?}"),
    }
}

#[test]
fn bracket_call_and_literal_construction_produce_the_same_variant_identity() {
    let module = compile(&format!(
        r#"{MARKED_OPTION}
micro produce() {{
    let bracketed = Some(1)
    let literal = Some {{ value: 1 }}
}}
"#
    ));
    let calls = resolved_calls_in_function(&module, "produce");
    assert_eq!(calls.len(), 2, "括号调用与字面量构造必须都解析成功");
    assert_eq!(calls[0].sum_variant, calls[1].sum_variant, "两种写法必须是同一变体声明");
    assert_eq!(calls[0].sum_owner, calls[1].sum_owner, "两种写法必须归属同一 unite 声明");
    assert_eq!(calls[0].return_type, calls[1].return_type, "两种写法的产物类型必须一致");
    assert_eq!(calls[0].domain, HirCallableDomain::Constructor);
    assert_eq!(calls[1].domain, HirCallableDomain::Constructor);
}

/// 私有载荷字段是「字面量构造始终可用」的唯一例外。
///
/// 语言已经建模 `[private]` 字段可见性（`HirField.visibility` + `lower_visibility`），
/// 但 `AccessControlChecker` / `VisibilityChecker` 目前没有任何调用点，字面量构造路径
/// 尚无可见性门禁。这里如实记录缺口：不发明可见性机制、不伪造通过。
#[test]
#[ignore = "blocked: 私有载荷字段的字面量构造尚无可见性语义支撑（`HirField.visibility` 已建模，`AccessControlChecker`/`VisibilityChecker` 无调用点；06 条目 30）"]
fn private_payload_field_literal_construction_is_rejected() {
    let error = compile_error(
        r#"
unite Secret<T> {
    [constructor]
    Hidden {
        [private]
        value: T,
    },
    Empty,
}
micro produce() {
    let literal = Hidden { value: 1 }
}
"#,
    );
    assert!(format!("{error:?}").contains("private"), "私有载荷字段的具名字面量构造必须结构化拒绝（当前尚无可见性门禁）: {error:?}");
}

#[test]
fn same_package_unite_named_construction_with_annotation_resolves() {
    let module = compile(
        r#"
unite Code {
    [constructor]
    Span {
        code: i64,
    },
}
micro produce() {
    let value: Code = Span { code: 2 }
}
"#,
    );
    let calls = resolved_calls_in_function(&module, "produce");
    assert_eq!(calls.len(), 1, "`Span {{ code: 2 }}` 必须产生一次已解析构造");
    assert_eq!(calls[0].domain, HirCallableDomain::Constructor);
    assert!(calls[0].sum_owner.is_some() && calls[0].sum_variant.is_some(), "具名构造必须携带 sum_owner/sum_variant");
    let function = module.functions.iter().find(|function| function.name.as_str() == "produce").expect("缺少 produce");
    let HirStatementKind::Let { initializer: Some(initializer), .. } = &function.body.statements[0].kind else {
        panic!("首个语句必须是带初始化的 let");
    };
    let HirExprKind::Construct { args, .. } = &initializer.kind else {
        panic!("初始化器必须是具名构造");
    };
    assert!(
        args.iter().all(|arg| matches!(&arg.kind, HirExprKind::FieldInit { field_id: Some(_), .. })),
        "具名构造的字段身份必须在解析期写回 `FieldInit.field_id`"
    );
}

#[test]
fn bare_variant_argument_gets_constructor_identity() {
    let module = compile(
        r#"
unite Outcome<T, E> {
    [constructor]
    Good {
        value: T,
    },
    [constructor]
    Bad {
        value: E,
    },
}
unite MathError {
    DivisionByZero,
    Overflow,
}
micro produce() -> Outcome<i64, MathError> {
    let outcome: Outcome<i64, MathError> = Bad(DivisionByZero)
    return outcome
}
"#,
    );
    let calls = resolved_calls_in_function(&module, "produce");
    let last = |name: &str| calls.iter().any(|call| call.symbol.parts().last().map(|part| part.as_str()) == Some(name));
    assert!(last("Bad"), "外层 `Bad(...)` 必须解析成功: {calls:?}");
    assert!(last("DivisionByZero"), "裸变体实参必须被改写成带 resolved 的构造: {calls:?}");
}
