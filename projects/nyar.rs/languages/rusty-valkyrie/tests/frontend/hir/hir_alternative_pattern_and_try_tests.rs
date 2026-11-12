//! `case A | B:` 备选模式与 `expr?` 后缀传播的 HIR 回归。
//!
//! 两者都是语言文档已声明的形态，HIR/MIR 侧语义早已存在：
//! - 备选模式降低为既有的 `HirPattern::Or`；
//! - `expr?` 降低为既有的 `HirExprKind::TryPropagate`（ok 变体留待 HIR 校验解析）。
//!
//! 本文件只锁“语法入口 → HIR 节点”的连通性，不重复测 MIR 语义。

use rusty_valkyrie::{
    ValkyrieCompiler,
    types::hir::{HirExprKind, HirPattern, HirStatementKind},
};

fn first_let_initializer_kind(source: &str) -> HirExprKind {
    let hir = ValkyrieCompiler::default().compile_source(source).expect("源码必须完成 HIR lowering");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "probe").expect("缺少 probe 函数");
    let statement = function.body.statements.first().expect("缺少 let 语句");
    let HirStatementKind::Let { initializer: Some(initializer), .. } = &statement.kind
    else {
        panic!("首语句必须是带初始化的 let：{:?}", statement.kind)
    };
    initializer.kind.clone()
}

#[test]
fn alternative_match_pattern_lowers_to_hir_or() {
    let kind = first_let_initializer_kind(
        "micro probe(value) { let out = match value { case 1 | 2 | 3: 10 else: 0 } }",
    );
    let HirExprKind::Match { arms, .. } = kind
    else {
        panic!("match 必须降低为 Match，实际 {kind:?}")
    };
    match &arms[0].pattern {
        HirPattern::Or(items) => assert_eq!(items.len(), 3, "三个备选必须全部保留"),
        other => panic!("备选模式必须降低为 HirPattern::Or，实际 {other:?}"),
    }
}

#[test]
fn try_propagate_postfix_lowers_to_hir_node() {
    let source = r#"
[primitive("std::primitive::i32")]
structure i32 { }

unite ProbeResult<T, E> {
    Ok { value: T }
    Err { error: E }
}

micro probe(input: ProbeResult<i32, i32>): ProbeResult<i32, i32> {
    let out = input?
    return input
}
"#;
    let kind = first_let_initializer_kind(source);
    assert!(matches!(kind, HirExprKind::TryPropagate { .. }), "`expr?` 必须降低为 TryPropagate，实际 {kind:?}");
}
