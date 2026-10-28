//! 泛型接收者的 trait 方法调用在 Semantic MIR 必须表示为 witness 分派义务，不得退回 `Unit`。

use rusty_valkyrie::compiler::types::hir::ValkyrieType;
use rusty_valkyrie::mir::{MirLowerer, MirOperand, MirOperation};
use rusty_valkyrie::ValkyrieCompiler;

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
    let method_declaration = echo_trait
        .methods
        .iter()
        .find(|method| method.name.as_str() == "echo")
        .expect("echo 方法声明")
        .declaration
        .expect("方法声明身份");
    let declaration = hir
        .functions
        .iter()
        .find(|function| function.name.as_str() == "use_echo")
        .and_then(|function| function.declaration)
        .expect("use_echo 声明身份");

    let mir = MirLowerer::lower_module_semantic(&hir);
    let function =
        mir.functions.iter().find(|function| function.declaration == Some(declaration)).expect("use_echo 必须有 Semantic MIR 函数");
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
        function.blocks.iter().flat_map(|block| block.instructions.iter()).any(|instruction| {
            matches!(&instruction.kind, MirOperation::Call { callee: MirOperand::Witness(_), .. })
        })
    });
    assert!(has_witness, "合同校验后 witness 分派必须仍然保留");
}

#[test]
fn witness_dispatch_reports_structured_canonical_failure() {
    // witness 在 canonical 没有对应形状：必须显式失败，不得退回 `Unit`，也不得按名称猜函数。
    let hir = ValkyrieCompiler::default().compile_source(SOURCE).expect("源码必须通过 HIR 分析");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let error =
        rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&mir).expect_err("witness 分派必须显式失败");
    let codes: Vec<_> = error.records.iter().map(|record| record.code.clone()).collect();
    assert!(codes.iter().any(|code| code == "CAN057"), "unexpected canonical diagnostics: {codes:?}");
}
