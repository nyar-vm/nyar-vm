#![cfg(feature = "ovm-lane")]
//! `[ovm_operator]` 无符号比较助记符的冻结表正反合同。
//!
//! 正向：`i32.lt_u`/`i32.le_u`/`i32.gt_u`/`i32.ge_u` 在 `TargetExecutionPlan`
//! 里冻结为二元 `Operands` 配方与对应无符号头码，backend 只消费计划事实。
//! 反向：表外助记符冻结为 `Unfrozen`，emit 必须结构化失败，不得回落到
//! 有符号头码或宿主导入。
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    target_operator::{TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorRecipe},
};
use odin_bytecode::{OdinHeadCode, OdinModuleData, decode_at};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

fn source_groups(mnemonic: &str) -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source(
            "core",
            "core",
            r#"[primitive("std::primitive::i32")]
structure i32 { }"#,
        ),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            &format!(
                r#"
[host_contract]
[ovm_operator("{mnemonic}")]
micro frozen_op(a: i32, b: i32): i32

[main]
micro main() -> i32 {{
    return frozen_op(2, 3)
}}
"#
            ),
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ]
}

fn plan_with_binding(mnemonic: &str) -> TargetExecutionPlan {
    let groups = source_groups(mnemonic);
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], "native")
        .expect("源码闭包必须完成 Compiler 链路");
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots).expect("计划必须可构造")
}

fn emitted_codes(module: &OdinModuleData) -> Vec<OdinHeadCode> {
    let mut codes = Vec::new();
    let mut position = 0;
    while position < module.code_bytes.len() {
        let instruction = decode_at(&module.code_bytes, position);
        assert!(instruction.size > 0, "字节码必须可解码");
        codes.push(instruction.code);
        position += instruction.size as usize;
    }
    codes
}

#[test]
fn unsigned_comparison_mnemonics_freeze_to_unsigned_head_codes() {
    for (mnemonic, code) in [
        ("i32.lt_u", OdinHeadCode::I32LtU),
        ("i32.le_u", OdinHeadCode::I32LeU),
        ("i32.gt_u", OdinHeadCode::I32GtU),
        ("i32.ge_u", OdinHeadCode::I32GeU),
    ] {
        let plan = plan_with_binding(mnemonic);
        assert_eq!(plan.imports().len(), 1, "只应有一个目标指令绑定导入");
        let import = plan.imports().values().next().expect("导入必须存在");
        let Some(TargetOperatorBinding::Frozen(operator)) = import.target_operator
        else {
            panic!("`{mnemonic}` 必须在计划里冻结成完整配方，got {:?}", import.target_operator);
        };
        assert_eq!(operator.arity, 2);
        assert_eq!(operator.operand_type, TargetOperandType::I32);
        assert_eq!(operator.result_type, TargetOperandType::I32);
        assert_eq!(operator.recipe, TargetOperatorRecipe::Operands);
        assert_eq!(operator.encoding, TargetOperatorEncoding::Ovm(code));

        let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
            .expect("已冻结配方必须可编码");
        assert!(emitted_codes(&module).contains(&code), "backend 必须发射 {code:?}，不得回落有符号变体");
    }
}

#[test]
fn unknown_ovm_mnemonic_stays_unfrozen_and_fails_structurally() {
    let plan = plan_with_binding("i32.lt_uu");
    let import = plan.imports().values().next().expect("导入必须存在");
    assert_eq!(import.target_operator, Some(TargetOperatorBinding::Unfrozen), "表外助记符不得被冻结为任何指令");

    let error = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .expect_err("缺配方不得编码为默认指令");
    let message = error.to_string();
    assert!(message.contains("i32.lt_uu"), "错误必须指出未冻结助记符: {message}");
}
