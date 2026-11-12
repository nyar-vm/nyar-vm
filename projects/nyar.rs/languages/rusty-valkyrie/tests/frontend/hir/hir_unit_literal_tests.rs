//! 单元字面量 `()` 是 `unit` 值的唯一写法，必须在 Oak 语法与 HIR 之间无损传递。
//!
//! 回归背景：空括号曾被 parser 标成 `LiteralExpression`，但 builder 没有对应
//! token，导致整份源码报 `Missing literal value`；修复后 parser 产出独立的
//! `UnitLiteral` 节点，HIR 映射到 `HirLiteral::Unit`。

use rusty_valkyrie::{
    ValkyrieCompiler,
    types::hir::{HirExprKind, HirLiteral, HirStatementKind},
};

#[test]
fn unit_literal_lowers_to_hir_unit() {
    let hir = ValkyrieCompiler::default().compile_source("micro probe() { let value = () }").expect("`()` 必须完成解析与 HIR lowering");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "probe").expect("缺少 probe 函数");
    let statement = function.body.statements.first().expect("缺少 let 语句");
    let HirStatementKind::Let { initializer: Some(initializer), .. } = &statement.kind
    else {
        panic!("首语句必须是带初始化的 let：{:?}", statement.kind)
    };
    assert!(matches!(initializer.kind, HirExprKind::Literal(HirLiteral::Unit)), "`()` 必须归 unit 字面量，实际 {:?}", initializer.kind);
}

#[test]
fn unit_literal_is_accepted_as_call_argument() {
    let source = r#"
micro take(value: unit): unit {
}

micro probe(): unit {
    take(())
}
"#;
    ValkyrieCompiler::default().compile_source(source).expect("`()` 作为实参必须完成 HIR lowering");
}
