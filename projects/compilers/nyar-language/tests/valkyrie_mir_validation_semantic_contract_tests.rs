use nyar_language::{
    MirBlock, MirBlockRef, MirConstant, MirTerminator, MirValueRef,
    mir::{MirInstruction, MirModule, MirOperand, MirOperation, MirSumDeclaration, MirSumVariant, ssa::MirExternalCallContract},
    types::{
        Identifier, NamePath,
        hir::{HirExprKind, ValkyrieType},
    },
    valkyrie::{mir::validation::validate_module, validation::validate_semantic_module},
};

fn empty_module() -> MirModule {
    nyar_language::mir::ssa::test_support::lower_test_module(Vec::new(), Vec::new())
}

fn source_identity_module(actual: &ValkyrieType, expected: &ValkyrieType) -> MirModule {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source("micro identity(value: bool) -> bool { return value }")
        .expect("当前源码解析后构造类型合同负向输入");
    let mut module = nyar_language::MirLowerer::lower_module_semantic(&hir);
    let function = &mut module.functions[0];
    function.param_types = vec![actual.clone()];
    for ty in function.value_types.values_mut() {
        *ty = actual.clone();
    }
    function.return_type = expected.clone();
    module
}

fn mismatched_nominal_contracts() -> Vec<(ValkyrieType, ValkyrieType)> {
    let applied = |name, arguments| ValkyrieType::Apply(Box::new(ValkyrieType::Named(Identifier::new(name))), arguments);
    vec![
        (applied("FirstResult", vec![ValkyrieType::Boolean]), applied("SecondResult", vec![ValkyrieType::Boolean])),
        (
            applied("Result", vec![ValkyrieType::Boolean, ValkyrieType::Utf8]),
            applied("Result", vec![ValkyrieType::Boolean, ValkyrieType::Utf16]),
        ),
        (applied("Option", vec![ValkyrieType::Boolean]), applied("Nullable", vec![ValkyrieType::Boolean])),
        (ValkyrieType::Integer32 { signed: true }, ValkyrieType::Named(Identifier::new("usize"))),
        (ValkyrieType::Utf16, ValkyrieType::Named(Identifier::new("Utf16Text"))),
    ]
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn option_none_generic_call_lowers_in_return_context() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace test;
unite Option<T> {
    Some { value: T },
    None,
}
micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}
micro take(flag: bool) -> Option<i64> {
    if flag {
        return option_none::<i64>()
    }
    let value: i64 = 1
    return Some(value)
}
"#,
        )
        .expect("option_none return shape must parse");
    let take_hir = hir.functions.iter().find(|function| function.name.as_str() == "take").expect("take hir");
    fn walk_option_none(expr: &nyar_language::hir::HirExpr, out: &mut Option<String>) {
        match &expr.kind {
            nyar_language::hir::HirExprKind::Call { callee, resolved, .. } => {
                let name = match &callee.kind {
                    nyar_language::hir::HirExprKind::GenericApply { callee, .. } => match &callee.kind {
                        nyar_language::hir::HirExprKind::Variable(ident) => ident.name.to_string(),
                        _ => "?".to_string(),
                    },
                    nyar_language::hir::HirExprKind::Variable(ident) => ident.name.to_string(),
                    _ => "?".to_string(),
                };
                if name == "option_none" {
                    *out = resolved
                        .as_ref()
                        .map(|call| format!("{:?} sum={}/{}", call.domain, call.sum_owner.is_some(), call.sum_variant.is_some()));
                }
            }
            nyar_language::hir::HirExprKind::Return(Some(inner)) => walk_option_none(inner, out),
            nyar_language::hir::HirExprKind::If { condition, then_branch, else_branch, .. } => {
                walk_option_none(condition, out);
                if let Some(expr) = &then_branch.expr {
                    walk_option_none(expr, out);
                }
                for statement in &then_branch.statements {
                    if let nyar_language::hir::HirStatementKind::Expr(expr) = &statement.kind {
                        walk_option_none(expr, out);
                    }
                }
                if let Some(else_branch) = else_branch {
                    if let Some(expr) = &else_branch.expr {
                        walk_option_none(expr, out);
                    }
                }
            }
            _ => {}
        }
    }
    let mut option_none_resolved = None;
    for statement in &take_hir.body.statements {
        if let nyar_language::hir::HirStatementKind::Expr(expr) = &statement.kind {
            walk_option_none(expr, &mut option_none_resolved);
        }
    }
    assert!(
        option_none_resolved.as_ref().is_some_and(|line| line.contains("Function") && !line.contains("sum=true/true")),
        "option_none in take must resolve as callable function, got {:?}",
        option_none_resolved
    );
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    assert!(mir.diagnostics.is_empty(), "mir diagnostics: {:?}", mir.diagnostics);
    validate_semantic_module(&mir).expect("option_none return must satisfy function return contract");
}

#[test]
fn i32_pow_while_if_loop_lowers_without_smir007() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
micro __i32_pow(lhs: i32, rhs: i32): i32 {
    let mut base: i32 = lhs
    let mut exp: i32 = rhs
    let mut acc: i32 = 1
    while true {
        if true {
            acc = acc
        }
        base = base
        exp = exp
    }
    return acc
}"#,
        )
        .expect("parse i32_pow fixture");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("__i32_pow while/if loop must satisfy SMIR contract");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn ascii_alnum_key_option_none_lowers_in_early_return() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}
namespace core::text;
[primitive("core::text::char")]
structure char { }
imply char {
    micro is_ascii_alphanumeric(self): bool { return false }
    micro ascii_alnum_key(self) -> Option<i64> {
        if self.is_ascii_alphanumeric() {
            return Some(1 as i64)
        }
        return option_none::<i64>()
    }
}
"#,
        )
        .expect("ascii_alnum_key shape must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("ascii_alnum_key must lower option_none early return");
    assert!(
        mir.functions.iter().any(|function| function.symbol.contains("option_none#")),
        "option_none::<i64> 必须物化为独立 ItemInstanceId（符号带 #）"
    );
    nyar_language::valkyrie::compile_pipeline::canonical_program_from_semantic_mir(&mir)
        .expect("ascii_alnum_key option_none monomorph 必须通过 Canonical CAN015");
}

#[test]
fn char_as_u32_cast_reaches_canonical_without_copy_type_theft() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::primitive;
[primitive("core::primitive::u32")]
structure u32 { }
namespace core::text;
[primitive("core::text::char")]
structure char { }
imply char {
    micro to_u32(self): u32 {
        return self as u32
    }
}
"#,
        )
        .expect("char as u32 fixture must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("char as u32 must satisfy SMIR");
    let has_cast = mir.functions.iter().any(|function| {
        function.blocks.iter().any(|block| block.instructions.iter().any(|instruction| matches!(instruction.kind, MirOperation::Cast { .. })))
    });
    assert!(has_cast, "self as u32 必须降低为 MirOperation::Cast，不得伪装 Copy");
    nyar_language::valkyrie::compile_pipeline::canonical_program_from_semantic_mir(&mir).expect("char as u32 Cast 必须通过 Canonical CAN015");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn lowercase_ascii_index_shape_validates_with_intrinsic_u32_sub() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> { Some { value: T }, None }
micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}
namespace core::primitive;
structure u32 { }
imply u32 {
    infix `-`(self, rhs: u32): u32 { __u32_sub(self, rhs) }
    infix `<`(self, rhs: u32): bool { __u32_lt(self, rhs) }
    infix `>`(self, rhs: u32): bool { __u32_gt(self, rhs) }
}
[intrinsic("u32.sub")]
micro __u32_sub(lhs: u32, rhs: u32): u32 { }
[intrinsic("u32.lt")]
micro __u32_lt(lhs: u32, rhs: u32): bool { }
[intrinsic("u32.gt")]
micro __u32_gt(lhs: u32, rhs: u32): bool { }
namespace core::text;
[primitive("core::text::char")]
structure char { }
imply char {
    micro lowercase_ascii_index(self) -> Option<i64> {
        let code: u32 = self as u32
        if code < 97 {
            return option_none::<i64>()
        }
        if code > 122 {
            return option_none::<i64>()
        }
        return Some((code - 97) as i64)
    }
}
"#,
        )
        .expect("lowercase_ascii_index intrinsic shape must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("lowercase_ascii_index intrinsic shape must validate");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn option_is_some_match_lowers_to_boolean_merge() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
imply Option<T> {
    micro is_some(self): bool {
        match self {
            case Some(value):
                true
            case None:
                false
        }
    }
}
"#,
        )
        .expect("Option.is_some match shape must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("Option.is_some must merge bool arms without Unit phi");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn option_unwrap_match_returns_payload_or_panics() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
micro panic<T>(message: text): Never {
    raise message
}
imply Option<T> {
    micro unwrap(self): T {
        match self {
            case Some(value):
                value
            case None:
                panic("unwrap on `None` value")
        }
    }
}
"#,
        )
        .expect("Option.unwrap match shape must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("Option.unwrap must bind Some payload and merge T arms");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn option_unwrap_or_else_indirect_call_lowers_function_parameter() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
imply Option<T> {
    micro unwrap_or_else(self, f: micro() -> T): T {
        match self {
            case Some(value):
                value
            case None:
                f()
        }
    }
}
"#,
        )
        .expect("Option.unwrap_or_else indirect call shape must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("Option.unwrap_or_else must indirect-call the function parameter");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn builtin_panic_call_lowers_to_uncaught_raise() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
micro panic<T>(message: text): Never {
    raise message
}
imply Option<T> {
    micro unwrap(self): T {
        match self {
            case Some(value):
                value
            case None:
                panic("unwrap on `None` value")
        }
    }
}
"#,
        )
        .expect("Option.unwrap panic arm must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    let function = mir.functions.iter().find(|function| function.symbol.contains("unwrap")).expect("unwrap mir");
    let has_raise_effect = function.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            nyar_language::valkyrie::mir::ssa::MirTerminator::PerformEffect {
                effect: nyar_language::valkyrie::mir::ssa::MirEffectKind::Raise,
                ..
            }
        )
    });
    assert!(has_raise_effect, "panic must lower to an uncaught raise effect");
    validate_module(&mir).expect("builtin panic lowering must not leave unresolved callable diagnostics");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn result_map_method_generic_return_uses_u_and_keeps_e() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Result<T, E> {
    Fine { value: T },
    Fail { error: E },
}
imply Result<T, E> {
    micro map<U>(self, f: micro(T) -> U): Result<U, E> {
        match self {
            case Fine(value):
                Fine(f(value))
            case Fail(error):
                Fail(error)
        }
    }
}
"#,
        )
        .expect("Result.map generic return must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("Result.map must lower Fine/Fail arms to Apply<Result<U, E>>");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn option_map_method_generic_return_uses_u_not_impl_t() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}
imply Option<T> {
    micro map<U>(self, f: micro(T) -> U): Option<U> {
        match self {
            case Some(value):
                Some(f(value))
            case None:
                option_none::<U>()
        }
    }
}
"#,
        )
        .expect("Option.map generic return must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("Option.map must return Apply<Option<U>>");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn option_flatten_nested_match_returns_inner_option() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
imply Option<T> {
    micro flatten(self: Option<Option<T>>): Option<T> {
        match self {
            case Some(value):
                value
            case None:
                None
        }
    }
}
"#,
        )
        .expect("Option.flatten nested match must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("Option.flatten must return Option<T> from nested Some arm");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn option_filter_guarded_match_returns_option() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
imply Option<T> {
    micro filter(self, predicate: micro(T) -> bool): Option<T> {
        match self {
            case Some(value) if predicate(value):
                Some(value)
            else:
                None
        }
    }
}
"#,
        )
        .expect("Option.filter guarded match must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("Option.filter must return Option<T>");
}

#[test]
#[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
fn option_or_else_indirect_call_merges_apply_option() {
    let hir = nyar_language::ValkyrieCompiler::default()
        .compile_source(
            r#"
namespace core::types;
unite Option<T> {
    Some { value: T },
    None,
}
micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}
imply Option<T> {
    micro or_else(self, f: micro() -> Option<T>): Option<T> {
        match self {
            case Some(value):
                Some(value)
            case None:
                f()
        }
    }
}
"#,
        )
        .expect("Option.or_else indirect call shape must parse");
    let mir = nyar_language::MirLowerer::lower_module_semantic(&hir);
    validate_semantic_module(&mir).expect("Option.or_else must merge Apply<Option<T>> arms");
}

#[test]
fn semantic_return_requires_exact_resolved_type_without_name_or_carrier_recovery() {
    validate_semantic_module(&source_identity_module(&ValkyrieType::Boolean, &ValkyrieType::Boolean)).expect("当前源码的完整 bool 返回合同");
    for (actual, expected) in mismatched_nominal_contracts() {
        let error = validate_semantic_module(&source_identity_module(&actual, &expected))
            .expect_err("同名模式、首个 payload 或物理表示相同都不能替代类型合同");
        assert_eq!(error.code, "SMIR007");
        assert!(error.detail.starts_with("return operand type"));
    }
}

#[test]
fn semantic_jump_requires_exact_resolved_block_parameter_type() {
    for (actual, expected) in mismatched_nominal_contracts() {
        let mut module = source_identity_module(&actual, &actual);
        let function = &mut module.functions[0];
        let source = function.blocks[0].parameters[0];
        let parameter = MirValueRef(function.value_types.keys().map(|value| value.0).max().unwrap() + 1);
        function.return_type = ValkyrieType::Unit;
        function.value_types.insert(parameter, expected);
        function.blocks[0].terminator = MirTerminator::Jump { target: MirBlockRef(1), arguments: vec![MirOperand::Value(source)] };
        function.blocks.push(MirBlock {
            id: MirBlockRef(1),
            label: "destination".into(),
            parameters: vec![parameter],
            instructions: Vec::new(),
            terminator: MirTerminator::Return { value: None },
        });
        let error = validate_semantic_module(&module).expect_err("块参数不能按名称或物理类型恢复");
        assert_eq!(error.code, "SMIR007");
        assert!(error.detail.starts_with("jump argument type"));
    }
}

#[test]
fn semantic_contract_rejects_nominal_sum_without_variants() {
    let mut module = empty_module();
    module.sum_types.push(MirSumDeclaration {
        nominal: nyar_types::NominalInstanceId::from_index(0).expect("测试 nominal identity"),
        declaration: None,
        name: "Choice".to_string(),
        is_unite: true,
        generics: Vec::new(),
        variants: Vec::new(),
    });

    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR006");
}

#[test]
fn semantic_contract_rejects_duplicate_nominal_sum_variant_tag() {
    let mut module = empty_module();
    module.sum_types.push(MirSumDeclaration {
        nominal: nyar_types::NominalInstanceId::from_index(0).expect("测试 nominal identity"),
        declaration: None,
        name: "Choice".to_string(),
        is_unite: false,
        generics: Vec::new(),
        variants: vec![
            MirSumVariant {
                id: nyar_types::VariantId::from_index(0).expect("测试 variant identity"),
                declaration: None,
                name: "First".to_string(),
                tag: 0,
                fields: Vec::new(),
                result_type: None,
            },
            MirSumVariant {
                id: nyar_types::VariantId::from_index(1).expect("测试 variant identity"),
                declaration: None,
                name: "Second".to_string(),
                tag: 0,
                fields: Vec::new(),
                result_type: None,
            },
        ],
    });

    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR006");
}

fn module_with_static_call(symbol: NamePath) -> MirModule {
    let mut module = empty_module();
    let mut function = nyar_language::mir::ssa::test_support::lower_test_function(nyar_language::mir::ssa::test_support::expr(
        HirExprKind::Literal(nyar_language::types::hir::HirLiteral::Bool(true)),
    ));
    function.blocks[0]
        .instructions
        .push(MirInstruction::from_operation(MirOperation::Call { callee: MirOperand::Symbol(symbol), arguments: Vec::new() }));
    module.functions.push(function);
    module
}

#[test]
fn semantic_contract_rejects_unresolved_static_call() {
    let module = module_with_static_call(NamePath::new(vec![Identifier::new("dependency"), Identifier::new("run")]));
    let error = validate_semantic_module(&module).unwrap_err();
    assert_eq!(error.code, "SMIR003");
    assert_eq!(error.location, "block 0 instruction 1");
}

#[test]
fn semantic_contract_checks_local_call_arguments_and_results() {
    let mut module = module_with_static_call(NamePath::new(vec![Identifier::new("target")]));
    let mut target = module.functions[0].clone();
    target.symbol = "target".to_owned();
    target.return_type = ValkyrieType::Unit;
    target.blocks[0].instructions.clear();
    target.blocks[0].terminator = MirTerminator::Return { value: None };
    module.functions.push(target);
    validate_semantic_module(&module).expect("exact zero-argument unit call");
    module.functions[1].param_types.push(ValkyrieType::Boolean);
    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR007");
    if let MirOperation::Call { arguments, .. } = &mut module.functions[0].blocks[0].instructions[1].kind {
        arguments.push(MirOperand::Constant(MirConstant::Unit));
    }
    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR007");
    if let MirOperation::Call { arguments, .. } = &mut module.functions[0].blocks[0].instructions[1].kind {
        arguments[0] = MirOperand::Constant(MirConstant::Bool(true));
    }
    validate_semantic_module(&module).expect("exact boolean argument");
    module.functions[1].return_type = ValkyrieType::Boolean;
    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR007");
}

#[test]
fn semantic_contract_rejects_duplicate_local_callable_identity() {
    let mut module = module_with_static_call(NamePath::new(vec![Identifier::new("target")]));
    let mut target = module.functions[0].clone();
    target.symbol = "target".to_owned();
    module.functions.push(target.clone());
    module.functions.push(target);
    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR003");
}

#[test]
fn semantic_contract_accepts_language_operator_call_without_registry_entry() {
    let module = module_with_static_call(NamePath::new(vec![Identifier::new("primitive"), Identifier::new("infix +")]));
    validate_semantic_module(&module).unwrap();
}

#[test]
fn semantic_contract_accepts_exact_dependency_export_call() {
    let symbol = NamePath::new(vec![Identifier::new("dependency"), Identifier::new("run")]);
    let mut module = module_with_static_call(symbol.clone());
    module.external_calls.push(MirExternalCallContract {
        declaration: None,
        instance: None,
        symbol,
        link: nyar_types::ExternalImportLink::host(None, vec!["dependency".to_owned(), "run".to_owned()]),
        parameter_types: Vec::new(),
        return_type: ValkyrieType::Unit,
    });

    validate_semantic_module(&module).unwrap();
    module.external_calls[0].parameter_types.push(ValkyrieType::Boolean);
    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR007");
    if let MirOperation::Call { arguments, .. } = &mut module.functions[0].blocks[0].instructions[1].kind {
        arguments.push(MirOperand::Constant(MirConstant::Bool(true)));
    }
    validate_semantic_module(&module).expect("exact external argument contract");
    let mut definition = module.functions[0].clone();
    definition.symbol = module.external_calls[0].symbol.to_string();
    definition.param_types = vec![ValkyrieType::Boolean];
    definition.return_type = ValkyrieType::Unit;
    definition.blocks[0].instructions.clear();
    definition.blocks[0].terminator = MirTerminator::Return { value: None };
    module.functions.push(definition);
    validate_semantic_module(&module).expect("matching linked declaration and definition");
    module.functions[1].param_types.clear();
    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR007");
    module.functions.pop();
    let dependency = module.clone();
    let before = module.clone();
    assert!(nyar_language::valkyrie::compile_pipeline::link_reachable_dependency_mir(&mut module, &[dependency.clone(), dependency]).is_err());
    assert_eq!(module, before);
    module.external_calls[0].return_type = ValkyrieType::Boolean;
    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR007");
    module.external_calls[0].return_type = ValkyrieType::Unit;
    module.external_calls.push(module.external_calls[0].clone());
    assert_eq!(validate_semantic_module(&module).unwrap_err().code, "SMIR003");
}
