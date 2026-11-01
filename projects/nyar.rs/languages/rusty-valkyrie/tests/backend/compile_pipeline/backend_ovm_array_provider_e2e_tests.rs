//! OVM 数组宿主 provider 的端到端合同。
//!
//! 锁定事实：泛型 `[host_contract]` 数组合同由 Resolver 绑定到泛型
//! `[host_provider]`（`[ovm("array.*")]` 宿主导入）后，合同调用点的具体代入
//! 在单态化同一固定点铸造 provider 闭合副本；降低产物只留 `nyar.host` 数组
//! 导入，由 odin-vm 内置宿主 op 实际执行并返回真实结果。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use odin_bytecode::OdinModuleData;
use odin_vm::{OdinVm, Value};
use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::{emitter::pipeline::CompilerHostProviderBinding, semantic::closure::micro_declaration_key},
};

const CORE_PRIMITIVE_SOURCE: &str = r#"[primitive("std::primitive::i32")]
structure i32 { }"#;

/// 泛型数组 provider：无体，目标事实由 `[ovm("array.*")]` 宿主导入携带。
const ARRAY_PROVIDER_SOURCE: &str = r#"
[host_provider(demo::array_new)]
[ovm("array.new")]
micro ovm_array_new<T>(length: i32): [T]

[host_provider(demo::array_length)]
[ovm("array.length")]
micro ovm_array_length<T>(xs: [T]): i32

[host_provider(demo::array_get)]
[ovm("array.get")]
micro ovm_array_get<T>(xs: [T], index: i32): T

[host_provider(demo::array_set)]
[ovm("array.set")]
micro ovm_array_set<T>(xs: [T], index: i32, value: T): unit
"#;

/// 泛型数组合同：无体，实现来源只能是 Resolver 选定的 provider。
const ARRAY_CONTRACT_SOURCE: &str = r#"
[host_contract]
micro array_new<T>(length: i32): [T]

[host_contract]
micro array_length<T>(xs: [T]): i32

[host_contract]
micro array_get<T>(xs: [T], index: i32): T

[host_contract]
micro array_set<T>(xs: [T], index: i32, value: T): unit
"#;

fn binding(contract: &str, provider: &str) -> CompilerHostProviderBinding {
    CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &[], contract),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], provider),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }
}

fn array_bindings() -> Vec<CompilerHostProviderBinding> {
    vec![
        binding("array_new", "ovm_array_new"),
        binding("array_length", "ovm_array_length"),
        binding("array_get", "ovm_array_get"),
        binding("array_set", "ovm_array_set"),
    ]
}

fn array_groups(main_body: &str) -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", CORE_PRIMITIVE_SOURCE),
        CompilerSourceGroup::single_source("adaptor", "adaptor", ARRAY_PROVIDER_SOURCE).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            &format!("{ARRAY_CONTRACT_SOURCE}\n[main]\nmicro main() -> i32 {{\n{main_body}\n}}\n"),
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ]
}

/// 沿 Compiler 产出的目标私有计划降低到 `.nyar`，不重放调用边。
fn lower_with_bindings(groups: &[CompilerSourceGroup], bindings: &[CompilerHostProviderBinding]) -> Result<OdinModuleData, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, bindings, "native")
        .map_err(|error| error.to_string())?;
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())
}

/// 编译 → 降低 → 编码 → odin-vm 内置运行时执行 `main`。
fn run_main(groups: &[CompilerSourceGroup], bindings: &[CompilerHostProviderBinding]) -> Value {
    let module = lower_with_bindings(groups, bindings).expect("数组 provider 链必须完成降低");
    let entry = module
        .exports
        .iter()
        .map(|export| export.symbol_name.clone())
        .find(|name| name.ends_with("main"))
        .unwrap_or_else(|| panic!("必须导出 main 入口: {:?}", module.exports));
    let bytes = odin_bytecode::encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("编码后的模块必须可加载");
    vm.run(&loaded, &entry, Vec::new()).expect("main 必须在内置 OVM 运行时执行成功")
}

#[test]
fn ovm_array_provider_executes_new_set_get_end_to_end() {
    let groups = array_groups("    let xs = array_new::<i32>(3)\n    array_set::<i32>(xs, 1, 42)\n    return array_get::<i32>(xs, 1)");
    let module = lower_with_bindings(&groups, &array_bindings()).expect("数组 provider 链必须完成降低");
    for symbol in ["array.new", "array.set", "array.get"] {
        assert!(
            module.imports.iter().any(|import| import.module_name == "nyar.host" && import.symbol_name == symbol),
            "被调用的 `{symbol}` 必须留下 nyar.host 导入: {:?}",
            module.imports
        );
    }
    assert!(
        module.imports.iter().all(|import| import.symbol_name != "array.length"),
        "未被调用的 `array.length` 不得进入导入表: {:?}",
        module.imports
    );
    assert_eq!(run_main(&groups, &array_bindings()), Value::I32(42), "new/set/get 链必须返回写入的元素");
}

#[test]
fn ovm_array_provider_executes_length_end_to_end() {
    let groups = array_groups("    let xs = array_new::<i32>(3)\n    return array_length::<i32>(xs)");
    assert_eq!(run_main(&groups, &array_bindings()), Value::I32(3), "array.length 必须返回分配长度");
}

/// 已解析调用指向未绑定 provider 的泛型数组合同必须在编译边界 fail-closed，
/// 不得静默编码成空体或默认值。
#[test]
fn resolved_generic_array_contract_without_provider_fails_at_boundary() {
    let groups = array_groups("    let xs = array_new::<i32>(3)\n    return array_length::<i32>(xs)");
    let message = lower_with_bindings(&groups, &[]).expect_err("未绑定 provider 的已解析合同调用必须失败");
    assert!(message.contains("no host provider is bound"), "expected compile-boundary gate failure, got: {message}");
    assert!(message.contains("array_new"), "failure must name the contract identity, got: {message}");
}
