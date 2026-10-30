//! `compile_linked_semantic_mir` 的诊断必须携带 Semantic MIR 合同自身的机器码与结构化字段。

use nyar_types::CompileStage;
use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::emitter::pipeline::compile_linked_semantic_mir,
    mir::{MirOperation, validate_semantic_module},
};

/// 源码构造合法的 `SumNew`，抽掉类型实参制造 SMIR006；管线诊断必须沿用合同码而非 PIPE004。
#[test]
fn semantic_contract_failure_reports_smir_code_not_pipe004() {
    let mut mir = ValkyrieCompiler::default()
        .compile_source_to_mir(
            r#"
unite Envelope<T> { Present { value: T }, Absent }
micro wrap(value: utf8) -> Envelope<utf8> {
    return Present { value: value }
}
"#,
        )
        .expect("源码必须构造出静态 sum 实例");
    validate_semantic_module(&mir).expect("未篡改前必须通过合同校验");
    let type_args = mir
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match &mut instruction.kind {
            MirOperation::SumNew { type_args, .. } => Some(type_args),
            _ => None,
        })
        .expect("源码 SumNew 指令");
    type_args.clear();
    let contract_error = validate_semantic_module(&mir).expect_err("缺类型实参必须违反 sum 构造合同");

    let failures = compile_linked_semantic_mir(&mir).expect_err("管线必须拒绝违反的 Semantic MIR");
    assert_eq!(failures.records.len(), 1, "单点合同违反只产生一条诊断");
    let record = &failures.records[0];
    assert_eq!(record.code, contract_error.code, "诊断码必须沿用合同自身机器码");
    assert_eq!(record.code, "SMIR006");
    assert_ne!(record.code, "PIPE004");
    assert_eq!(record.stage, CompileStage::ValidateMir);
    assert_eq!(record.module, mir.name);
    for field in [&contract_error.function, &contract_error.location, &contract_error.detail] {
        assert!(record.message.contains(field.as_str()), "消息必须包含结构化字段 `{field}`，实际为 `{}`", record.message);
    }
    assert!(!record.message.contains("SemanticMirContractError"), "不得转储 Debug 结构: {}", record.message);
}
