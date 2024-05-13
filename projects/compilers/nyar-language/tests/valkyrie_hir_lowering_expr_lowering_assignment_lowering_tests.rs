use nyar_language::{
    types::{
        SourceID,
        hir::{HirExpr, HirExprKind},
    },
    valkyrie::{
        frontend::{self, ast::Statement},
        hir::lowering::expr_lowering::lower_term_expression,
    },
};

fn lower(source: &str) -> HirExpr {
    let root = frontend::parse_source(source).expect("完整 AST");
    let frontend::ast::StatementNode::Micro(function) = &root.items[0]
    else {
        panic!("缺少函数")
    };
    let Statement::Expression(statement) = &function.body.statements[0]
    else {
        panic!("缺少表达式")
    };
    lower_term_expression(&statement.expr, SourceID::default(), 0..source.len())
}

#[test]
fn assignment_and_field_write_keep_structured_targets() {
    assert!(matches!(lower("micro update() { value = 7 }").kind, HirExprKind::Assign { .. }));
    assert!(matches!(lower("micro update() { object.field = 7 }").kind, HirExprKind::StoreField { .. }));
}

#[test]
fn iteration_source_is_not_a_boolean_condition() {
    assert!(matches!(
        lower("micro visit() { loop item in values { consume(item) } }").kind,
        HirExprKind::Loop { pattern: Some(_), iterator: Some(_), condition: None, .. }
    ));
    assert!(matches!(
        lower("micro visit() { while ready { consume() } }").kind,
        HirExprKind::Loop { pattern: None, iterator: None, condition: Some(_), .. }
    ));
}

#[test]
fn bitwise_or_is_an_operator_call_not_a_pipeline() {
    for (source, lexeme) in [
        ("micro bits() { left | right }", "|"),
        ("micro bits() { left & right }", "&"),
        ("micro bits() { left << right }", "<<"),
        ("micro bits() { left >> right }", ">>"),
    ] {
        let HirExprKind::Call { call_kind: HirCallKind::Operator(operator), args, .. } = lower(source).kind
        else {
            panic!("按位表达式缺少运算符身份")
        };
        assert_eq!(operator, registered_operator(OperatorFixity::Infix, lexeme));
        assert_eq!(args.len(), 2);
    }
}

#[test]
fn ordinal_and_cardinal_access_bind_distinct_operators() {
    for (source, lexeme, arity) in [
        ("micro read() { values[1] }", "[]", 2),
        ("micro read() { values⁅0⁆ }", "⁅⁆", 2),
        ("micro read() { values::[0] }", "⁅⁆", 2),
        ("micro write() { values[1] = 7 }", "[]=", 3),
        ("micro write() { values⁅0⁆ = 7 }", "⁅⁆=", 3),
    ] {
        let HirExprKind::Call { call_kind: HirCallKind::Operator(operator), args, .. } = lower(source).kind
        else {
            panic!("下标缺少运算符合同")
        };
        assert_eq!(operator, registered_operator(OperatorFixity::Postfix, lexeme));
        assert_eq!(args.len(), arity);
    }
}
