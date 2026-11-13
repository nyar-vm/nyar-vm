//! 泛型接收者的 trait 方法调用在 Semantic MIR 必须表示为 witness 分派义务，不得退回 `Unit`。

use nyar_types::{ItemId, ItemInstanceId};
use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::types::hir::ValkyrieType,
    mir::{MirLowerer, MirOperand, MirOperation},
};

const SOURCE: &str = r#"
namespace test.dispatch;

trait Echo {
    micro echo(self) -> bool
}

micro use_echo<T>(value: T) -> bool where T: Echo {
    return value.echo()
}
"#;

#[test]
fn generic_receiver_trait_call_keeps_witness_dispatch() {
    let hir = ValkyrieCompiler::default().compile_source(SOURCE).expect("带 where 约束的泛型调用必须可解析");
    let echo_trait = hir.traits.iter().find(|item| item.name.as_str() == "Echo").expect("Echo 声明");
    let trait_declaration = echo_trait.declaration.expect("trait 声明身份");
    let method_declaration =
        echo_trait.methods.iter().find(|method| method.name.as_str() == "echo").expect("echo 方法声明").declaration.expect("方法声明身份");
    let declaration = hir
        .functions
        .iter()
        .find(|function| function.name.as_str() == "use_echo")
        .and_then(|function| function.declaration)
        .expect("use_echo 声明身份");

    let mir = MirLowerer::lower_module_semantic(&hir);
    let function = mir.functions.iter().find(|function| function.declaration == Some(declaration)).expect("use_echo 必须有 Semantic MIR 函数");
    let (witness, results) = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .find_map(|instruction| match &instruction.kind {
            MirOperation::Call { callee: MirOperand::Witness(witness), .. } => Some((witness, &instruction.results)),
            _ => None,
        })
        .expect("泛型 trait 方法调用必须是 witness 分派，不得降级为 `Unit`");

    assert_eq!(witness.trait_declaration, trait_declaration);
    assert_eq!(witness.method_declaration, method_declaration);
    match &witness.self_type {
        ValkyrieType::Generic(generic) => assert!(generic.binder.is_some(), "开放泛型必须保留 binder"),
        other => panic!("expected open Generic(T), got {other:?}"),
    }
    let result = results.first().expect("声明有返回类型的方法调用必须占用 SSA 结果槽");
    assert_eq!(function.value_types.get(result), Some(&ValkyrieType::Boolean), "调用结果类型必须是声明返回类型，不得退回 `Unit`");
}

#[test]
fn witness_call_satisfies_semantic_mir_contract() {
    // witness 分派必须能通过 Semantic MIR 合同校验：既不能判为非法静态调用，
    // 也不能因为缺少静态签名而被拒绝。
    let mir = ValkyrieCompiler::default().compile_source_to_mir(SOURCE).expect("witness 分派必须通过 Semantic MIR 合同校验");
    let has_witness = mir.functions.iter().any(|function| {
        function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .any(|instruction| matches!(&instruction.kind, MirOperation::Call { callee: MirOperand::Witness(_), .. }))
    });
    assert!(has_witness, "合同校验后 witness 分派必须仍然保留");
}

#[test]
fn canonical_rejects_witness_without_frozen_instance() {
    // 具体 witness 必须在实例化固定点消去为冻结 `ItemInstanceId`。这个合成模块
    // 没有登记 `method_declaration` 对应的实现实例，canonical 必须结构化拒绝，
    // 不得保留 `Witness` 让目标层扫描实例库决定调用谁。
    let self_type = ValkyrieType::Integer32 { signed: true };
    let module = concrete_witness_module(self_type.clone());

    let error = rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&module)
        .expect_err("缺少冻结实现实例的 witness 必须在 canonical 失败闭合");
    let rendered = format!("{error:?}");
    assert!(rendered.contains("UnresolvedWitnessDispatch"), "拒绝必须指出未消去的 witness 分派: {rendered}");
}

/// 构造一个非泛型函数体，其中唯一的调用是已具体化 self 的 trait 方法槽 witness。
fn concrete_witness_module(self_type: ValkyrieType) -> rusty_valkyrie::mir::MirModule {
    use rusty_valkyrie::mir::{
        AggregateLayoutPlan, MirBlock, MirBlockRef, MirFunction, MirInstruction, MirModule, MirTraitWitness, MirValue, MirValueOrigin,
        MirValueRef,
    };

    let receiver = MirValueRef(0);
    let result = MirValueRef(1);
    let mut module = MirModule {
        name: "demo".into(),
        functions: vec![MirFunction {
            display_symbol: "demo::main".into(),
            symbol: rusty_valkyrie::types::NamePath::new(vec![
                rusty_valkyrie::types::Identifier::new("demo"),
                rusty_valkyrie::types::Identifier::new("main"),
            ]),
            declaration: Some(ItemId::from_index(0).expect("声明身份")),
            instance: Some(ItemInstanceId::from_index(0).expect("实例身份")),
            return_type: ValkyrieType::Boolean,
            param_types: vec![self_type.clone()],
            value_types: std::collections::BTreeMap::from([(receiver, self_type.clone()), (result, ValkyrieType::Boolean)]),
            entry: MirBlockRef(0),
            values: vec![
                MirValue { id: receiver, origin: MirValueOrigin::Parameter { index: 0, name: "self".into() } },
                MirValue { id: result, origin: MirValueOrigin::Temporary },
            ],
            blocks: vec![MirBlock {
                id: MirBlockRef(0),
                label: "entry".into(),
                parameters: vec![receiver],
                instructions: vec![MirInstruction::from_operation_with_results(
                    MirOperation::Call {
                        callee: MirOperand::Witness(MirTraitWitness {
                            trait_declaration: ItemId::from_index(1).expect("trait 声明身份"),
                            method_declaration: ItemId::from_index(2).expect("方法槽身份"),
                            self_type,
                        }),
                        arguments: vec![MirOperand::Value(receiver)],
                    },
                    vec![result],
                )],
                terminator: rusty_valkyrie::MirTerminator::Return { value: Some(MirOperand::Value(result)) },
            }],
        }],
        structs: Vec::new(),
        imports: Vec::new(),
        external_calls: Vec::new(),
        exports: Vec::new(),
        entries: Vec::new(),
        callable_symbols: std::collections::BTreeMap::from([(ItemInstanceId::from_index(0).expect("实例身份"), "demo::main".to_owned())]),
        type_identities: std::collections::BTreeMap::new(),
        aggregate_layouts: AggregateLayoutPlan::default(),
        sum_types: Vec::new(),
        flags_types: Vec::new(),
        singleton_instances: Vec::new(),
        semantic_fragments: Vec::new(),
        diagnostics: Vec::new(),
    };
    module.type_identities =
        rusty_valkyrie::compiler::mir::ssa::type_identity_table(&module.functions, &module.external_calls, &module.structs, &module.sum_types);
    module
}

const CONCRETE_DISPATCH_SOURCE: &str = r#"
namespace test.dispatch;

trait Echo {
    micro echo(self) -> bool
}

structure Counter {
    flag: bool
}

imply Counter: Echo {
    micro echo(self) -> bool {
        return true
    }
}

micro use_echo<T>(value: T) -> bool where T: Echo {
    return value.echo()
}

[export]
micro run() -> bool {
    return use_echo(Counter { flag: true })
}
"#;

#[test]
fn concrete_self_without_frozen_instance_is_rejected_not_resolved_later() {
    // 具体 self 类型不足以消去 witness：必须同时存在冻结的实现实例。
    // 这个 fixture 的 impl 成员没有进入实例库，canonical 必须结构化拒绝，
    // 不得把「按 self 类型再找一次实现」推迟到目标层。
    let hir = ValkyrieCompiler::default().compile_source(CONCRETE_DISPATCH_SOURCE).expect("具体 trait 实现必须通过 HIR 分析");
    let mir = MirLowerer::lower_module_semantic(&hir);

    let error = rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&mir)
        .expect_err("没有冻结实现实例的具体 witness 必须失败闭合");
    let rendered = format!("{error:?}");
    assert!(rendered.contains("UnresolvedWitnessDispatch"), "拒绝必须指出未消去的 witness 分派: {rendered}");
    assert!(rendered.contains("candidates: 0"), "缺实例必须报告候选数为 0: {rendered}");
}

const OPERATOR_SOURCE: &str = r#"
namespace test.dispatch;

trait Comparer {
    infix `==`(self, other: Self): bool
}

micro same<T>(left: T, right: T) -> bool where T: Comparer {
    return left == right
}
"#;

#[test]
fn operator_trait_call_keeps_witness_dispatch() {
    // `a == b` 在 HIR 展开为单段 Path callee（`infix ==`）、接收者位于首位实参。
    // 这条真实路径同样必须产出 witness 分派，并且结果类型取声明返回类型。
    let hir = ValkyrieCompiler::default().compile_source(OPERATOR_SOURCE).expect("带 where 约束的运算符调用必须可解析");
    let comparer = hir.traits.iter().find(|item| item.name.as_str() == "Comparer").expect("Comparer 声明");
    let trait_declaration = comparer.declaration.expect("trait 声明身份");
    let method_declaration = comparer.methods.first().expect("运算符方法声明").declaration.expect("方法声明身份");
    let declaration = hir
        .functions
        .iter()
        .find(|function| function.name.as_str() == "same")
        .and_then(|function| function.declaration)
        .expect("same 声明身份");

    let mir = MirLowerer::lower_module_semantic(&hir);
    let function = mir.functions.iter().find(|function| function.declaration == Some(declaration)).expect("same 必须有 Semantic MIR 函数");
    let (witness, results) = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .find_map(|instruction| match &instruction.kind {
            MirOperation::Call { callee: MirOperand::Witness(witness), .. } => Some((witness, &instruction.results)),
            _ => None,
        })
        .expect("运算符形式的 trait 方法调用必须是 witness 分派，不得降级为 `Unit`");

    assert_eq!(witness.trait_declaration, trait_declaration);
    assert_eq!(witness.method_declaration, method_declaration);
    let result = results.first().expect("声明有返回类型的方法调用必须占用 SSA 结果槽");
    assert_eq!(function.value_types.get(result), Some(&ValkyrieType::Boolean), "调用结果类型必须是声明返回类型，不得退回 `Unit`");
}

#[test]
fn open_generic_witness_is_rejected_at_canonical() {
    // 泛型形参是前端冻结的 binder 身份（MIR 层事实）；但开放 self 的 witness
    // 没有唯一实现实例，canonical 必须在实例化固定点结构化拒绝，不得保留它让
    // 目标层扫描实例库，也不得按名字猜实现。
    let hir = ValkyrieCompiler::default().compile_source(SOURCE).expect("源码必须通过 HIR 分析");
    let mir = MirLowerer::lower_module_semantic(&hir);

    let (generic, binder) = mir
        .type_identities
        .keys()
        .find_map(|ty| match ty {
            ValkyrieType::Generic(generic) if generic.name.as_str() == "T" => Some((ty.clone(), generic.binder)),
            _ => None,
        })
        .expect("形参 T 必须进入 Compiler 类型身份表");
    let binder = binder.expect("形参 T 必须携带 binder 身份");
    // MIR 层：witness 的 self 类型就是带同一声明 binder 的形参，不按名字重建。
    let function = mir.functions.iter().find(|function| function.display_symbol.contains("use_echo")).expect("use_echo MIR");
    let witness_self = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .find_map(|instruction| match &instruction.kind {
            MirOperation::Call { callee: MirOperand::Witness(witness), .. } => Some(witness.self_type.clone()),
            _ => None,
        })
        .expect("use_echo 必须保留 witness 分派");
    match witness_self {
        ValkyrieType::Generic(generic) => {
            assert_eq!(generic.binder, Some(binder), "witness self 必须落在声明 binder 身份上，而不是按名字重建");
            assert_eq!(generic.name.as_str(), "T");
        }
        other => panic!("expected Generic(T), got {other:?}"),
    }

    let error = rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&mir)
        .expect_err("开放 self 的 witness 必须在 canonical 失败闭合");
    let rendered = format!("{error:?}");
    assert!(rendered.contains("UnresolvedWitnessDispatch"), "拒绝必须指出未消去的 witness 分派: {rendered}");
}
