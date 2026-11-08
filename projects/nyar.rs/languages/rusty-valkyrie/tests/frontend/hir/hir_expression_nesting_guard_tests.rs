//! 表达式 lowering 嵌套深度守卫：普通嵌套必须成功，超过上限必须结构化失败。

use rusty_valkyrie::{HirError, ValkyrieCompiler, compiler::hir::lowering::MAX_EXPRESSION_LOWERING_DEPTH};

use crate::primitives::SourceFixtureCompile;

fn nested_source(depth: usize) -> String {
    let mut source = String::from("micro deep(): i64 {\n    return ");
    source.push_str(&"(".repeat(depth));
    source.push('1');
    source.push_str(&")".repeat(depth));
    source.push_str("\n}\n");
    source
}

/// 正例：三层括号的普通表达式必须完成 lowering。
#[test]
fn shallow_expression_lowers() {
    let source = nested_source(3);
    ValkyrieCompiler::default().compile_fixture_source(&source).expect("浅层嵌套表达式必须成功");
}

/// 反例：超过深度上限必须返回结构化 `ExpressionNestingExceeded`，不得栈溢出。
#[test]
fn deeply_nested_expression_fails_closed() {
    let source = nested_source(MAX_EXPRESSION_LOWERING_DEPTH + 2);
    let error = ValkyrieCompiler::default().compile_source(&source).expect_err("超过深度上限必须失败");
    match error {
        HirError::ExpressionNestingExceeded { depth, limit, .. } => {
            assert!(depth > limit, "depth={depth} 必须大于 limit={limit}");
            assert_eq!(limit, MAX_EXPRESSION_LOWERING_DEPTH);
        }
        other => panic!("期望 ExpressionNestingExceeded，实际 {other:?}"),
    }
}
