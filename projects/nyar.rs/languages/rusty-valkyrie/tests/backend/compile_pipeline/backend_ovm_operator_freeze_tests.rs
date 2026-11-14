//! `[ovm_operator("...")]` 助记符冻结表的活体合同。
//!
//! 锁定事实：冻结表覆盖的全部助记符沿 Compiler → `TargetExecutionPlan` → OVM
//! backend 降低为对应单指令，不得留下 `nyar.host` 导入；未知助记符与 arity
//! 不符都必须结构化失败，禁止回落宿主导入或默认指令。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use acorn_odin::{OdinHeadCode, OdinModuleData, decode_at};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

/// 沿 Compiler 产出的目标私有计划降低到 `.odin`，不重放调用边。
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
        let instruction = acorn_odin::decode_at(&module.code_bytes, position);
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
            r#"namespace std.primitive;

[primitive("std::primitive::i32")]
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
            r#"namespace std.primitive;

[primitive("std::primitive::i32")]
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
            r#"namespace std.primitive;

[primitive("std::primitive::i32")]
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

        let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
            .expect("已冻结配方必须可编码");
        assert!(emitted_codes(&module).contains(&code), "backend 必须发射 {code:?}，不得回落有符号变体");
    }
}

#[test]
fn unknown_ovm_mnemonic_stays_unfrozen_and_fails_structurally() {
    let plan = plan_with_binding("i32.lt_uu");
    assert_eq!(plan.imports().len(), 1, "导入必须存在");

    let error = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .expect_err("缺配方不得编码为默认指令");
    let message = error.to_string();
    assert!(message.contains("i32.lt_uu"), "错误必须指出未冻结助记符: {message}");
}

/// D15 夹具：共存 `i64 ==` 与 `bool ==` / `!=` 的源码组。
fn d15_groups(with_i64_eq: bool) -> Vec<CompilerSourceGroup> {
    let i64_impl = if with_i64_eq {
        r#"
imply i64 {
    infix `==`(self, rhs: Self): bool { return true }
}
"#
    }
    else {
        ""
    };
    vec![
        CompilerSourceGroup::single_source(
            "core",
            "core",
            &format!(
                r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 {{ }}

[primitive("std::primitive::bool")]
structure bool {{ }}

namespace demo;

imply bool {{
    prefix `!`(self): bool {{ return false }}
    infix `==`(self, rhs: Self): bool {{ return true }}
    infix `!=`(self, rhs: Self): bool {{ return !(self == rhs) }}
}}
{i64_impl}
"#
            ),
        ),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            &format!(
                r#"
namespace demo;

[export(case: "camelCase")]
micro probe(a: i64, b: i64, x: bool, y: bool) -> bool {{
    let neg: bool = x != y
    {i64_use}return neg
}}
"#,
                i64_use = if with_i64_eq { "let eq: bool = a == b\n    " } else { "" }
            ),
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ]
}

/// D15 夹具：把源码组降低到片段提交，供导出名与入口决议断言。
fn d15_submission(groups: &[CompilerSourceGroup]) -> FragmentSubmission {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, &[], &[], "native")
        .expect("源码闭包必须完成 Compiler 链路");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots).expect("计划必须冻结");
    FragmentSubmission::from_backend_plan(plan)
}

/// 内部 operator 实例（`imply bool { infix == }`）不是声明导出面：即使多个实例
/// 共享同一 ABI 名，也不得进入 `.odin` 导出表，更不该逼出「按实例下标消歧」。
#[test]
fn internal_operator_instances_stay_outside_the_export_surface() {
    let submission = d15_submission(&d15_groups(true));
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&submission).expect("内部 `==` 实例不得阻断降低");
    let names = module.exports.iter().map(|export| export.symbol_name.clone()).collect::<Vec<_>>();
    assert_eq!(names, vec!["probe".to_owned()], "只有声明的 `[export]` 进入导出面: {names:?}");
    let entries = nyar_emitter::lowering::backends::ovm::library_public_export_names(&submission);
    assert_eq!(entries, vec!["probe".to_owned()], "库公开入口只来自声明导出面");
    nyar_emitter::resolve_entry_symbols(&module, &entries, true).expect("库公开入口必须逐一解析到唯一函数导出");
}

/// 导出面里仍可能出现 abi 派生名与公共导出名同名的冲突（入口不是 `[export]` 时）。
/// 公共名是 run-contract 的稳定表面，只有非公共（入口）名追加实例下标。
#[test]
fn entry_abi_name_colliding_with_public_export_name_gains_instance_index() {
    let groups = d15_groups(false);
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
        .expect("源码闭包必须完成 Compiler 链路");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let mut plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots).expect("计划必须可构造");
    let eq = program
        .canonical()
        .linked
        .callable_names
        .iter()
        .find(|(_, name)| name.to_string().contains("infix =="))
        .map(|(instance, _)| *instance)
        .expect("bool `==` 实例");
    let probe = program
        .canonical()
        .linked
        .callable_names
        .iter()
        .find(|(_, name)| name.to_string().ends_with("probe"))
        .map(|(instance, _)| *instance)
        .expect("probe 实例");
    let eq_abi_name = plan.abi_name_for_instance(eq).expect("`==` 必须有 ABI 名");
    let eq_abi = eq_abi_name.to_string();
    plan.testing_export_names_mut().insert(probe, eq_abi.clone());
    plan.testing_entry_operation(eq_abi_name);
    let submission = FragmentSubmission::from_backend_plan(plan);
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&submission).expect("冲突导出面必须完成降低");
    let names = module.exports.iter().map(|export| export.symbol_name.clone()).collect::<Vec<_>>();
    assert!(names.iter().any(|name| name == &eq_abi), "公共导出名必须保持原样: {names:?}");
    assert!(names.iter().any(|name| name == &format!("{eq_abi}#{}", eq.index())), "非公共入口名必须按实例下标消歧: {names:?}");
    nyar_emitter::resolve_entry_symbols(&module, &nyar_emitter::lowering::backends::ovm::library_public_export_names(&submission), true)
        .expect("冲突消歧后每个公开入口仍必须唯一可解析");
}

/// D15 反例：不存在的入口名仍然必须按「唯一明确函数导出」失败闭合。
#[test]
fn unknown_entry_name_still_fails_closed() {
    let submission = d15_submission(&d15_groups(true));
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&submission).expect("共存 `==` 实例必须完成降低");
    let error = nyar_emitter::resolve_entry_symbols(&module, &["core::missing".to_owned()], true).expect_err("未知入口名必须结构化失败");
    assert!(error.contains("必须对应唯一明确函数导出"), "{error}");
}
