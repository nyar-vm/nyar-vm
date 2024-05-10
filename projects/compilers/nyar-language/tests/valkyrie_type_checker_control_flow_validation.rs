use nyar_language::{
    types::{
        Identifier, NamePath, SourceID, SourceSpan,
        hir::{HirExpr, HirExprKind},
    },
    valkyrie::{
        hir::HirResolvedCall,
        type_checker::{TypeError, TypeInference},
    },
};

fn span() -> SourceSpan {
    SourceSpan::new(SourceID::default(), 0, 0)
}

#[test]
fn unresolved_operator_name_cannot_supply_a_result_type() {
    let expression = HirExpr {
        kind: HirExprKind::Call {
            call_kind: nyar_language::types::hir::HirCallKind::Operator(nyar_types::builtin_operator::infix_eq()),
            callee: Box::new(HirExpr { kind: HirExprKind::Path(NamePath::new(vec![Identifier::new("infix ==")])), span: span() }),
            args: Vec::new(),
            resolved: None,
        },
        span: span(),
    };
    assert_eq!(TypeInference::new().infer(&expression), Err(TypeError::UnsupportedExpression));
}

#[test]
fn resolved_call_result_comes_from_its_signature() {
    let expression = HirExpr {
        kind: HirExprKind::Call {
            call_kind: nyar_language::types::hir::HirCallKind::Operator(nyar_types::builtin_operator::infix_add()),
            callee: Box::new(HirExpr { kind: HirExprKind::Path(NamePath::new(vec![Identifier::new("infix +")])), span: span() }),
            args: Vec::new(),
            resolved: Some(HirResolvedCall {
                declaration: None,
                instance: None,
                sum_owner: None,
                sum_variant: None,
                symbol: NamePath::new(vec![Identifier::new("declared_callable")]),
                domain: nyar_language::types::hir::HirCallableDomain::Operator,
                return_type: nyar_language::types::hir::ValkyrieType::Boolean,
                parameter_types: Vec::new(),
                has_receiver: false,
                extractor_payload_type: None,
            }),
        },
        span: span(),
    };
    assert_eq!(TypeInference::new().infer(&expression), Ok(nyar_language::types::hir::ValkyrieType::Boolean));
}
