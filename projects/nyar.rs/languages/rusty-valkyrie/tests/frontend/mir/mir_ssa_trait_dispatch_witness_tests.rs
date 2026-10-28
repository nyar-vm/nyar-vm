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
fn canonical_witness_slot_carries_dispatch_identity() {
    // canonical 必须保留 witness 的稳定身份（trait 声明 + 方法槽 + 已具体化 self 类型），
    // 不得整体拒绝，也不得按名字猜实现。
    let self_type = ValkyrieType::Integer32 { signed: true };
    let module = concrete_witness_module(self_type.clone());
    let expected_self = *module.type_identities.get(&self_type).expect("self 类型必须进入 identity 表");

    let program = rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&module)
        .expect("已具体化 self 的 witness 分派必须进入 canonical");
    let function = program.mir.functions.values().next().expect("canonical 必须保留函数体");
    let witness = function
        .blocks
        .values()
        .flat_map(|block| block.instructions.iter())
        .find_map(|instruction| match &instruction.operation {
            nyar_types::CanonicalOperation::Invoke { callee: nyar_types::CanonicalCallee::Witness(witness), .. } => Some(*witness),
            _ => None,
        })
        .expect("witness 分派必须保留在 canonical Invoke 上");

    assert_eq!(witness.trait_declaration, ItemId::from_index(1).expect("trait 声明身份"));
    assert_eq!(witness.method_declaration, ItemId::from_index(2).expect("方法槽身份"));
    assert_eq!(witness.self_type, Some(expected_self), "已具体化的 self 类型必须以 TypeId 进入 canonical");
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

#[test]
fn generic_witness_identity_is_not_concretized_before_canonical() {
    // 开放泛型的 self 类型身份尚未具体化：canonical 不猜 TypeId，也不退回 `Unit`；
    // 泛型中间表示进入 canonical 之前仍需闭合签名（当前边界）。
    let hir = ValkyrieCompiler::default().compile_source(SOURCE).expect("源码必须通过 HIR 分析");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let error = rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&mir)
        .expect_err("开放泛型签名尚未闭合时 canonical 必须显式失败");
    let codes: Vec<_> = error.records.iter().map(|record| record.code.clone()).collect();
    assert!(codes.iter().any(|code| code == "CAN014"), "unexpected canonical diagnostics: {codes:?}");
    assert!(!codes.iter().any(|code| code == "CAN057"), "witness 不得再被整体拒绝: {codes:?}");
}
