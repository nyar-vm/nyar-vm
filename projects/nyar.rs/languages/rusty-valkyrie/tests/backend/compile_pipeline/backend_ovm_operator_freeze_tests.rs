//! `[ovm_operator("...")]` 助记符冻结表的活体合同。
//!
//! 锁定事实：冻结表覆盖的全部助记符沿 Compiler → `TargetExecutionPlan` → OVM
//! backend 降低为对应单指令，不得留下 `nyar.host` 导入；未知助记符与 arity
//! 不符都必须结构化失败，禁止回落宿主导入或默认指令。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    target_operator::{TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorRecipe},
};
use odin_bytecode::{OdinHeadCode, OdinModuleData, decode_at};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

/// 沿 Compiler 产出的目标私有计划降低到 `.nyar`，不重放调用边。
fn try_lower_source_groups(groups: &[CompilerSourceGroup]) -> Result<OdinModuleData, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, &[], &[], "native")
        .expect("源码闭包必须完成 Compiler 链路");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())
}

fn decoded_codes(module: &OdinModuleData) -> Vec<OdinHeadCode> {
    let mut codes = Vec::new();
    let mut position = 0;
    while position < module.code_bytes.len() {
        let instruction = odin_bytecode::decode_at(&module.code_bytes, position);
        assert!(instruction.size > 0, "字节码必须可解码");
        codes.push(instruction.code);
        position += instruction.size as usize;
    }
    codes
}

fn operator_groups(mnemonic: &str, provider: &str) -> Vec<CompilerSourceGroup> {
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
micro {provider}(a: i32, b: i32): i32

[main]
micro main() -> i32 {{
    return {provider}(6, 3)
}}
"#
            ),
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ]
}

/// 冻结表全表：算术、位运算/移位、有符号与无符号比较助记符必须逐一冻结为
/// 对应单指令，且不得留下任何 `nyar.host` 导入。
#[test]
fn ovm_operator_freeze_table_lowers_to_vm_opcodes_without_host_import() {
    for (mnemonic, provider, expected) in [
        ("i32.add", "i32_add", OdinHeadCode::I32Add),
        ("i32.sub", "i32_sub", OdinHeadCode::I32Sub),
        ("i32.mul", "i32_mul", OdinHeadCode::I32Mul),
        ("i32.div", "i32_div", OdinHeadCode::I32DivS),
        ("i32.rem", "i32_rem", OdinHeadCode::I32RemS),
        ("i32.and", "i32_and", OdinHeadCode::I32And),
        ("i32.or", "i32_or", OdinHeadCode::I32Or),
        ("i32.xor", "i32_xor", OdinHeadCode::I32Xor),
        ("i32.shl", "i32_shl", OdinHeadCode::I32Shl),
        ("i32.shr_s", "i32_shr_s", OdinHeadCode::I32ShrS),
        ("i32.shr_u", "i32_shr_u", OdinHeadCode::I32ShrU),
        ("i32.eq", "i32_eq", OdinHeadCode::I32Eq),
        ("i32.ne", "i32_ne", OdinHeadCode::I32Ne),
        ("i32.lt", "i32_lt", OdinHeadCode::I32LtS),
        ("i32.le", "i32_le", OdinHeadCode::I32LeS),
        ("i32.gt", "i32_gt", OdinHeadCode::I32GtS),
        ("i32.ge", "i32_ge", OdinHeadCode::I32GeS),
        ("i32.lt_u", "i32_lt_u", OdinHeadCode::I32LtU),
        ("i32.le_u", "i32_le_u", OdinHeadCode::I32LeU),
        ("i32.gt_u", "i32_gt_u", OdinHeadCode::I32GtU),
        ("i32.ge_u", "i32_ge_u", OdinHeadCode::I32GeU),
    ] {
        let module = try_lower_source_groups(&operator_groups(mnemonic, provider)).expect("已冻结助记符必须完成降低");
        let codes = decoded_codes(&module);
        assert!(codes.contains(&expected), "`{mnemonic}` 必须降低为 {expected:?} 指令，实际 {codes:?}");
        assert!(
            module.imports.iter().all(|import| import.symbol_name != provider && import.module_name != "nyar.host"),
            "已内联的 `{provider}` 不得留下 nyar.host 导入: {:?}",
            module.imports
        );
    }
}

/// 未知 `ovm_operator` 助记符不得回落为宿主导入或默认指令，必须结构化失败。
#[test]
fn unknown_ovm_operator_mnemonic_fails_structurally() {
    let message = try_lower_source_groups(&operator_groups("i32.unknown", "i32_add")).expect_err("未知助记符必须结构化失败");
    assert!(message.contains("ovm_operator"), "{message}");
    assert!(message.contains("i32.unknown"), "{message}");
    assert!(message.contains("function="), "错误必须带函数符号: {message}");
}

/// 助记符 arity 与实参数量不符时不得发射指令，必须结构化失败。
#[test]
fn ovm_operator_arity_mismatch_fails_structurally() {
    let groups = vec![
        CompilerSourceGroup::single_source(
            "core",
            "core",
            r#"[primitive("std::primitive::i32")]
structure i32 { }"#,
        ),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            r#"
[host_contract]
[ovm_operator("i32.neg")]
micro i32_neg(a: i32, b: i32): i32

[main]
micro main() -> i32 {
    return i32_neg(1, 2)
}
"#,
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ];
    let message = try_lower_source_groups(&groups).expect_err("助记符 arity 与实参数量不符必须结构化失败");
    assert!(message.contains("i32.neg"), "{message}");
    assert!(message.contains("expected=1"), "错误必须带期望 arity: {message}");
    assert!(message.contains("actual=2"), "错误必须带实际实参数: {message}");
}

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
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
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
