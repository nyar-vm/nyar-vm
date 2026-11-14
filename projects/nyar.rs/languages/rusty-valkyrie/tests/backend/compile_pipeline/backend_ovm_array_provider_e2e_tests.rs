//! OVM 数组宿主 provider 的端到端合同。
//!
//! 锁定事实：泛型 `[host_contract]` 数组合同由 Resolver 绑定到泛型
//! `[host_provider]`（`[ovm("array.*")]` 宿主导入）后，合同调用点的具体代入
//! 在单态化同一固定点铸造 provider 闭合副本；降低产物只留 `nyar.host` 数组
//! 导入，由 odin-vm 内置宿主 op 实际执行并返回真实结果。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use acorn_odin::{OdinExportKind, OdinModuleData, encode_module};
use odin_vm::{OdinVm, Value, executor::Executor, module::LoadedModule};
use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::{emitter::pipeline::CompilerHostProviderBinding, semantic::closure::micro_declaration_key},
};

const CORE_PRIMITIVE_SOURCE: &str = r#"namespace std.primitive;

[primitive("std::primitive::i32")]
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

/// 沿 Compiler 产出的目标私有计划降低到 `.odin`，不重放调用边。
fn lower_with_bindings(groups: &[CompilerSourceGroup], bindings: &[CompilerHostProviderBinding]) -> Result<OdinModuleData, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, bindings, &[], "native")
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
    let bytes = acorn_odin::encode_module(&module);
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

const CONTRACTS: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

namespace demo.collection.array;

[host_contract]
micro new<T>(length: i32): [T]

[host_contract]
micro length<T>(array: [T]): i32

[host_contract]
micro get<T>(array: [T], index: i32): T

[host_contract]
micro set<T>(array: [T], index: i32, value: T): unit
"#;

// 与 std adaptor 正式形制一致：`[host_provider]` 包装体调用无体 `[ovm]` 宿主导入 micro。
// 泛型 provider 的具体代入由合同调用点在单态化播种（`host_provider_instantiation_seeds`）
// 中铸造，绑定按同签名闭合实例逐对配对重映射；provider 体内的 `[ovm]` 抽象调用随同一
// 固定点闭合并登记 external_call 合同，链接 BFS 与后端编码走同一条正式实例化路径。
const PROVIDERS: &str = r#"
namespace demo.adaptor.collection;

[host_provider(demo::collection::array::new)]
private micro array_host_new<T>(length: i32): [T] {
    return __array_ovm_new::<T>(length)
}

[host_provider(demo::collection::array::length)]
private micro array_host_length<T>(array: [T]): i32 {
    return __array_ovm_length::<T>(array)
}

[host_provider(demo::collection::array::get)]
private micro array_host_get<T>(array: [T], index: i32): T {
    return __array_ovm_get::<T>(array, index)
}

[host_provider(demo::collection::array::set)]
private micro array_host_set<T>(array: [T], index: i32, value: T): unit {
    __array_ovm_set::<T>(array, index, value)
}

[ovm("array.new"), pure]
private micro __array_ovm_new<T>(length: i32): [T] { }

[ovm("array.length"), pure]
private micro __array_ovm_length<T>(array: [T]): i32 { }

[ovm("array.get"), pure]
private micro __array_ovm_get<T>(array: [T], index: i32): T { }

[ovm("array.set")]
private micro __array_ovm_set<T>(array: [T], index: i32, value: T): unit { }
"#;

// 应用与合同同命名空间并使用无限定 turbofish：限定路径 + `::<T>` 的解析
// 正由 core 收尾会话修复（`type_lowering.rs`/`resolver.rs`），本测试不依赖该在途改动。
const APP: &str = r#"
namespace demo.collection.array;

micro pick_two() -> i32 {
    let xs: [i32] = new::<i32>(3)
    set(xs, 1, 42)
    return get(xs, 1)
}

micro sized() -> i32 {
    let xs: [i32] = new::<i32>(5)
    return length(xs)
}

[main]
micro main() -> i32 {
    let n = sized()
    return pick_two()
}
"#;

fn source_groups(with_providers: bool) -> Vec<CompilerSourceGroup> {
    let mut groups = vec![CompilerSourceGroup::single_source("core", "core", CONTRACTS)];
    let mut app_dependencies = vec!["core".to_owned()];
    if with_providers {
        groups.push(CompilerSourceGroup::single_source("adaptor", "adaptor", PROVIDERS).with_direct_dependencies(vec!["core".into()]));
        app_dependencies.push("adaptor".into());
    }
    groups.push(CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(app_dependencies));
    groups
}

/// 合同 → 选定 provider 的显式绑定（与 legion 规划器产出的 `CompilerHostProviderBinding` 同形制）。
fn provider_bindings() -> Vec<rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding> {
    use rusty_valkyrie::compiler::semantic::closure::micro_declaration_key;
    let array_ns = ["demo".to_owned(), "collection".to_owned(), "array".to_owned()];
    let adaptor_ns = ["demo".to_owned(), "adaptor".to_owned(), "collection".to_owned()];
    [("new", "array_host_new"), ("length", "array_host_length"), ("get", "array_host_get"), ("set", "array_host_set")]
        .into_iter()
        .map(|(contract, provider)| rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
            contract: micro_declaration_key("core", "core.v", &array_ns, contract),
            provider: micro_declaration_key("adaptor", "adaptor.v", &adaptor_ns, provider),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        })
        .collect()
}

fn lower(
    groups: &[CompilerSourceGroup],
    bindings: &[rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding],
    test_exports: &[&str],
) -> Result<acorn_odin::OdinModuleData, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, bindings, &[], "native")
        .map_err(|error| error.to_string())?;
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let mut plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    // 宿主只能调用声明导出面的成员；被测内部函数由测试显式登记为导出，而不是靠可达闭包。
    for symbol in test_exports {
        let instance = program
            .canonical()
            .linked
            .callable_names
            .iter()
            .find(|(_, name)| name.to_string().ends_with(symbol))
            .map(|(instance, _)| *instance)
            .ok_or_else(|| format!("缺少 `{symbol}` 的 callable 实例"))?;
        plan.testing_export_names_mut().insert(instance, (*symbol).to_string());
    }
    nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())
}

fn run_export(module: &acorn_odin::OdinModuleData, symbol_suffix: &str) -> Value {
    let bytes = encode_module(module);
    let loaded = LoadedModule::from_bytes(&bytes).expect("已绑定宿主导入的模块必须通过加载校验");
    let entry = loaded
        .exports
        .iter()
        .find(|export| export.kind == OdinExportKind::Function && export.symbol_name.ends_with(symbol_suffix))
        .unwrap_or_else(|| panic!("导出表必须包含 `{symbol_suffix}`"));
    let mut executor = Executor::new();
    executor.run(&loaded, entry.function_index as usize, Vec::new()).expect("执行")
}

#[test]
fn array_contract_provider_chain_executes_in_ovm() {
    let module = lower(&source_groups(true), &provider_bindings(), &["sized"]).expect("合同与 provider 齐备时必须完成降低");
    assert_eq!(run_export(&module, "main"), Value::I32(42), "new→set→get 必须读回写入的元素");
    assert_eq!(run_export(&module, "sized"), Value::I32(5), "new→length 必须返回分配长度");
}

// 形制 2：`[ovm]` 直接挂在无体 `[host_provider]` micro 上（无包装体）。
// provider 声明本身即宿主导入；播种铸造其闭合抽象副本，contracts 收集为 ovm
// 导入合同，绑定把闭合合同实例配到该副本并重映射，消费模块持导入合同编码。
const PROVIDERS_OVM_DIRECT: &str = r#"
namespace demo.adaptor.collection;

[host_provider(demo::collection::array::new), ovm("array.new"), pure]
private micro array_host_new<T>(length: i32): [T] { }

[host_provider(demo::collection::array::length), ovm("array.length"), pure]
private micro array_host_length<T>(array: [T]): i32 { }

[host_provider(demo::collection::array::get), ovm("array.get"), pure]
private micro array_host_get<T>(array: [T], index: i32): T { }

[host_provider(demo::collection::array::set), ovm("array.set")]
private micro array_host_set<T>(array: [T], index: i32, value: T): unit { }
"#;

#[test]
fn array_contract_ovm_direct_provider_form_executes_in_ovm() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", CONTRACTS),
        CompilerSourceGroup::single_source("adaptor", "adaptor", PROVIDERS_OVM_DIRECT).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let module = lower(&groups, &provider_bindings(), &["sized"]).expect("形制 2（`[ovm]` 直挂无体 provider）必须完成降低");
    assert_eq!(run_export(&module, "main"), Value::I32(42), "形制 2: new→set→get 必须读回写入的元素");
    assert_eq!(run_export(&module, "sized"), Value::I32(5), "形制 2: new→length 必须返回分配长度");
}

#[test]
fn array_contract_without_provider_fails_structurally() {
    let error = lower(&source_groups(false), &[], &[]).expect_err("缺 provider 的合同调用必须结构化失败，不得空体成功");
    assert!(error.contains("demo") && error.contains("array"), "错误必须指向未绑定 provider 的合同: {error}");
}
