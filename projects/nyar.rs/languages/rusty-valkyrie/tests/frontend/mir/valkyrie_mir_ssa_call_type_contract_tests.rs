use nyar_types::ItemInstanceId;
use rusty_valkyrie::{
    Identifier, NamePath, ValkyrieCompiler,
    mir::ssa::{MirConstant, MirDiagnostic, MirLowerer, MirModule, MirOperand, MirOperation, test_support::TestMirBuilder},
    types::hir::{HirExprKind, HirLiteral, HirPattern, ValkyrieType},
};

use crate::primitives::SourceFixtureCompile;

#[test]
fn missing_hir_static_contract_cannot_be_reconstructed_from_spelling() {
    let mut hir = ValkyrieCompiler::default()
        .compile_fixture_source("micro answer() -> i64 { 7 } micro caller() -> i64 { answer() }")
        .expect("源码必须先完成解析");
    let caller = hir.functions.iter_mut().find(|function| function.name.as_str() == "caller").unwrap();
    let HirExprKind::Call { resolved, .. } = &mut caller.body.expr.as_mut().unwrap().kind
    else {
        panic!("预期调用表达式");
    };
    assert!(resolved.take().is_some());
    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(mir.diagnostics.iter().any(|diagnostic| matches!(diagnostic, MirDiagnostic::UnresolvedCallableIdentity { .. })));
    let caller = mir.functions.iter().find(|function| function.display_symbol.ends_with("::caller")).unwrap();
    assert!(
        !caller.blocks.iter().flat_map(|block| &block.instructions).any(|instruction| matches!(instruction.kind, MirOperation::Call { .. }))
    );
    assert!(rusty_valkyrie::compiler::mir::validation::validate_semantic_module(&mir).is_err());
}

#[test]
fn resolved_callable_identity_survives_diagnostic_name_changes() {
    let mut hir = ValkyrieCompiler::default()
        .compile_fixture_source("micro answer() -> i64 { 7 } micro caller() -> i64 { answer() }")
        .expect("源码必须先完成解析");
    let caller = hir.functions.iter_mut().find(|function| function.name.as_str() == "caller").unwrap();
    let HirExprKind::Call { resolved: Some(resolved), .. } = &mut caller.body.expr.as_mut().unwrap().kind
    else {
        panic!("预期已解析调用");
    };
    let path = NamePath::new(vec![Identifier::new("Owner"), Identifier::new("infix +")]);
    let instance = resolved.instance.expect("源码解析必须持有实例身份");
    resolved.symbol = path.clone();
    let mir = MirLowerer::lower_module_semantic(&hir);
    let caller = mir.functions.iter().find(|function| function.display_symbol.ends_with("::caller")).unwrap();
    let callees = caller
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::Call { callee, .. } => Some(callee.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(callees, vec![MirOperand::Callable(instance)]);
}

#[test]
fn same_method_names_keep_declared_result_types() {
    let hir = ValkyrieCompiler::default()
        .compile_fixture_source(
            r#"
structure BooleanOwner { }
structure IntegerOwner { }

imply BooleanOwner {
    micro read() -> bool { return true }
}

imply IntegerOwner {
    micro read() -> i64 { return 7 }
}

micro read_boolean() -> bool { return BooleanOwner.read() }
micro read_integer() -> i64 { return IntegerOwner.read() }
"#,
        )
        .expect("同名不同 owner 的静态方法必须完成 HIR 解析");
    let mir = MirLowerer::lower_module_semantic(&hir);
    for (owner, expected) in [("BooleanOwner.read", ValkyrieType::Boolean), ("IntegerOwner.read", ValkyrieType::Integer64 { signed: true })] {
        let instance = mir
            .functions
            .iter()
            .find(|function| function.display_symbol.starts_with(owner))
            .and_then(|function| function.instance)
            .expect("被调用方法实例");
        let mut matches = 0;
        for function in &mir.functions {
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                if let MirOperation::Call { callee: MirOperand::Callable(callee), arguments } = &instruction.kind {
                    if *callee == instance {
                        matches += 1;
                        assert!(arguments.is_empty(), "无 self 声明不得增加接收者");
                        assert_eq!(instruction.results.len(), 1, "非 unit 返回值必须绑定结果");
                        assert_eq!(function.value_types.get(&instruction.results[0]), Some(&expected));
                    }
                }
            }
        }
        assert_eq!(matches, 1, "调用必须保持完整 owner 身份");
    }
}

#[test]
fn declared_operator_uses_its_callable_identity_and_signature() {
    let hir = ValkyrieCompiler::default()
        .compile_source(
            r#"
structure Number { }
imply Number {
    infix `+`(self, rhs: Number) -> bool { true }
}
micro apply(left: Number, right: Number) -> bool { left + right }
"#,
        )
        .expect("用户运算符必须经声明 overload 解析");
    let operator = &hir.impls[0].methods[0];
    let instance = operator.instance.expect("operator 声明实例身份");
    let apply = hir.functions.iter().find(|function| function.name.as_str() == "apply").expect("apply");
    let HirExprKind::Call { resolved: Some(resolved), .. } = &apply.body.expr.as_ref().expect("调用表达式").kind
    else {
        panic!("运算符调用必须绑定声明");
    };
    assert_eq!(resolved.symbol.to_string(), "Number.infix +", "运算符语法糖只保留 owner 限定的普通成员名");
    assert_eq!(resolved.instance, Some(instance));
    assert!(resolved.has_receiver, "receiver 由声明中的 self 参数决定");
    assert_eq!(resolved.return_type, ValkyrieType::Boolean);
    assert_eq!(resolved.parameter_types.len(), 2);

    let mir = MirLowerer::lower_module_semantic(&hir);
    let apply = mir.functions.iter().find(|function| function.display_symbol.ends_with("::apply")).expect("Semantic MIR apply");
    let calls = apply
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::Call { callee: MirOperand::Callable(callee), arguments } if *callee == instance => Some((instruction, arguments)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1.len(), 2, "显式参数包含签名声明的 self，不由语法补造");
    assert_eq!(apply.value_types.get(&calls[0].0.results[0]), Some(&ValkyrieType::Boolean));
}

#[test]
fn pattern_literal_compare_uses_dispatch_bound_infix_eq() {
    let owner = ValkyrieType::Named(Identifier::new("i64"));
    let eq_instance = ItemInstanceId::from_index(1).expect("instance");
    let mut builder = TestMirBuilder::new();
    builder.register_member_dispatch(owner.clone(), "infix ==", eq_instance);
    let lhs = builder.typed_value_operand(owner.clone());
    let _ = builder.lower_eq_constant_operand(lhs, MirConstant::Int(7), &owner);
    assert!(builder.diagnostics().is_empty());
    assert_eq!(builder.call_callees(), vec![MirOperand::Callable(eq_instance)]);
}

#[test]
fn pattern_range_compare_uses_dispatch_bound_infix_lt() {
    // 数值身份来自声明载体：合成 owner 用 `Integer64` 载体，不再用裸短名 `Named("i64")`。
    let owner = ValkyrieType::Integer64 { signed: true };
    let lt_instance = ItemInstanceId::from_index(2).expect("instance");
    let mut builder = TestMirBuilder::new();
    builder.register_member_dispatch(owner.clone(), "infix <", lt_instance);
    let value = builder.typed_value_operand(owner.clone());
    let pattern = HirPattern::Range { start: Some(HirLiteral::Integer(2)), end: Some(HirLiteral::Integer(8)), inclusive_end: true };
    let _ = builder.lower_pattern_match_operand(&pattern, value);
    let lt_calls = builder
        .call_callees()
        .into_iter()
        .filter(|callee| matches!(callee, MirOperand::Callable(instance) if *instance == lt_instance))
        .count();
    assert_eq!(lt_calls, 2, "inclusive range `2..=8` lowers to two infix < calls");
    assert!(builder.diagnostics().is_empty());
}

#[test]
fn integer_match_patterns_use_declared_operator_dispatch() {
    let hir = ValkyrieCompiler::default()
        .compile_source(
            r#"
[primitive("std::primitive::i64")]
structure i64 { }
imply i64 {
    infix `==`(self, rhs: Self): bool { return true }
    infix `<`(self, rhs: Self): bool { return true }
}
micro literal_match(value: i64) -> i64 {
    match value {
        case 7: 1
        else: 0
    }
}
micro range_match(value: i64) -> i64 {
    match value {
        case 2..=8: 1
        else: 0
    }
}
"#,
        )
        .expect("match patterns with declared operators must parse");
    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(
        !mir.diagnostics.iter().any(|diagnostic| matches!(diagnostic, MirDiagnostic::UnresolvedCallableIdentity { .. })),
        "pattern compares must bind Callable instances: {:?}",
        mir.diagnostics
    );
    let callable_calls = mir
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| &block.instructions)
        .filter(|instruction| matches!(instruction.kind, MirOperation::Call { callee: MirOperand::Callable(_), .. }))
        .count();
    assert!(callable_calls >= 3, "literal `==` plus inclusive range needs at least three operator calls, got {callable_calls}");
    rusty_valkyrie::compiler::mir::validation::validate_semantic_module(&mir).expect("match operator calls must pass semantic contract");
}

#[test]
fn integer_match_patterns_without_operators_fail_closed() {
    let hir = ValkyrieCompiler::default()
        .compile_source(
            r#"
[primitive("std::primitive::i64")]
structure i64 { }
micro literal_match(value: i64) -> i64 {
    match value {
        case 7: 1
        else: 0
    }
}
micro range_match(value: i64) -> i64 {
    match value {
        case 2..=8: 1
        else: 0
    }
}
"#,
        )
        .expect("match patterns without operators must still parse");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let mut unresolved = mir
        .diagnostics
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            MirDiagnostic::UnresolvedCallableIdentity { symbol } => Some(symbol.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    unresolved.sort();
    let mut expected = vec!["infix <".to_string(), "infix <".to_string(), "infix ==".to_string()];
    expected.sort();
    assert_eq!(unresolved, expected);
    assert!(rusty_valkyrie::compiler::mir::validation::validate_module(&mir).is_err());
}

#[test]
fn u32_infix_subtraction_call_matches_declared_return_type() {
    let hir = ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace std::primitive;
[primitive("std::primitive::u32")]
structure u32 { }
imply u32 {
    infix `-`(self, rhs: u32): u32 {
        return rhs
    }
}
micro demo(value: u32): u32 {
    return value - 1
}
"#,
        )
        .expect("u32 subtraction must parse");
    let mir = MirLowerer::lower_module_semantic(&hir);
    rusty_valkyrie::compiler::mir::validation::validate_semantic_module(&mir).expect("u32 subtraction call must match declared return type");
}

#[test]
fn some_short_name_without_sum_identity_fails_closed() {
    let mut hir = ValkyrieCompiler::default()
        .compile_fixture_source(
            "unite Option<T> { [constructor] Some { value: T }, None } \
             micro wrap(value: utf8) -> Option<utf8> { return Some(value) }",
        )
        .expect("Option/Some 源码必须完成 HIR");
    let wrap = hir.functions.iter_mut().find(|function| function.name.as_str() == "wrap").expect("wrap");
    let body = wrap.body.expr.as_mut().expect("wrap body");
    let call = match &mut body.kind {
        HirExprKind::Return(Some(expr)) => expr.as_mut(),
        HirExprKind::Call { .. } => body,
        other => panic!("预期 Some 调用或 return Some，得到 {other:?}"),
    };
    let HirExprKind::Call { resolved: Some(resolved), .. } = &mut call.kind
    else {
        panic!("预期 Some 调用表达式");
    };
    resolved.sum_owner = None;
    resolved.sum_variant = None;
    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(
        mir.diagnostics.iter().any(|diagnostic| matches!(diagnostic, MirDiagnostic::UnresolvedVariantIdentity { .. })),
        "缺 sum 身份的 Some 不得成功降低: {:?}",
        mir.diagnostics
    );
    assert!(
        !mir.functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .any(|instruction| { matches!(instruction.kind, MirOperation::SumNew { .. }) }),
        "缺身份时不得发出 SumNew"
    );
}

const INTEGER_COMPARISON_SOURCE: &str = r#"
namespace std::primitive;
structure i64 { }
imply i64 {
    infix `==`(self, rhs: Self): bool {
        return integer_bitwise_equal(self, rhs)
    }
}
micro integer_bitwise_equal(left: i64, right: i64): bool {
    return true
}
micro compare(left: i64, right: i64): bool {
    return left == right
}
"#;

fn integer_comparison_module() -> MirModule {
    let hir = ValkyrieCompiler::default().compile_source(INTEGER_COMPARISON_SOURCE).expect("整数比较源码必须完成 HIR");
    MirLowerer::lower_module_semantic(&hir)
}

/// 整数比较调用：声明返回 `bool`，结果 SSA 类型也是 `bool`，必须通过语义合同校验。
#[test]
fn integer_comparison_call_matches_declared_boolean_return_type() {
    let module = integer_comparison_module();
    let operator_instance = module
        .functions
        .iter()
        .find(|function| function.display_symbol.contains("infix =="))
        .and_then(|function| function.instance)
        .expect("`i64.infix ==` 必须持有实例身份");
    let operator = module
        .functions
        .iter()
        .find(|function| function.instance == Some(operator_instance))
        .expect("`i64.infix ==` 必须作为 Semantics MIR 函数存在");
    assert_eq!(operator.return_type, ValkyrieType::Boolean, "比较运算符的声明返回值是 `bool`");

    let (caller, call_result) = module
        .functions
        .iter()
        .find_map(|function| {
            let call_result = function.blocks.iter().flat_map(|block| &block.instructions).find_map(|instruction| match &instruction.kind {
                MirOperation::Call { callee: MirOperand::Callable(callee), .. } if *callee == operator_instance => {
                    instruction.results.first().copied()
                }
                _ => None,
            })?;
            Some((function, call_result))
        })
        .expect("`compare` 必须把 `left == right` 降低成 `Callable` 比较调用");
    assert_eq!(caller.value_types.get(&call_result), Some(&ValkyrieType::Boolean), "比较调用结果的 SSA 类型必须与声明的 `bool` 一致");

    rusty_valkyrie::compiler::mir::validation::validate_semantic_module(&module).expect("返回 `bool` 的整数比较调用必须通过语义合同");
}

/// 旧 provider 把比较声明成返回 `i32`：声明 `bool` 与 SSA 结果 `i32` 不同源时，校验必须整体失败。
#[test]
fn integer_comparison_call_rejects_i32_ssa_result_against_boolean_declaration() {
    let mut module = integer_comparison_module();
    let operator_instance = module
        .functions
        .iter()
        .find(|function| function.display_symbol.contains("infix =="))
        .and_then(|function| function.instance)
        .expect("`i64.infix ==` 必须持有实例身份");
    let operator = module
        .functions
        .iter_mut()
        .find(|function| function.instance == Some(operator_instance))
        .expect("`i64.infix ==` 必须作为 Semantics MIR 函数存在");
    assert_eq!(operator.return_type, ValkyrieType::Boolean, "比较运算符的声明返回值是 `bool`");
    let call_result = operator
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match &instruction.kind {
            MirOperation::Call { .. } => instruction.results.first().copied(),
            _ => None,
        })
        .expect("`i64.infix ==` 的 body 必须持有比较调用");
    operator.value_types.insert(call_result, ValkyrieType::Integer32 { signed: true });

    let error = rusty_valkyrie::compiler::mir::validation::validate_semantic_module(&module)
        .expect_err("声明 `bool` 而 SSA 结果 `i32` 的调用必须被拒绝");
    assert_eq!(error.code, "SMIR007");
    assert_eq!(error.detail, "call result type differs from declared signature");
    assert!(error.function.contains("i64.infix =="), "错误必须能定位到比较运算符函数，实际为 `{}`", error.function);

    let rendered = format!("{}: function={}, detail={}", error.code, error.function, error.detail);
    assert!(rendered.contains("SMIR007"), "{rendered}");
    assert!(rendered.contains("call result type differs from declared signature"), "{rendered}");
    assert!(rendered.contains("function=i64.infix =="), "{rendered}");
}

#[test]
fn integer_literal_equality_does_not_fabricate_a_false_result() {
    let mut builder = TestMirBuilder::new();
    let result = builder.lower_eq_constant_operand(
        MirOperand::Constant(rusty_valkyrie::compiler::mir::MirConstant::Int(1)),
        rusty_valkyrie::compiler::mir::MirConstant::Int(1),
        &ValkyrieType::Integer64 { signed: true },
    );
    assert_eq!(result, MirOperand::Constant(rusty_valkyrie::compiler::mir::MirConstant::Unit));
    assert!(matches!(
        builder.diagnostics(),
        [MirDiagnostic::UnresolvedCallableIdentity { symbol }] if symbol == "infix =="
    ));
}
