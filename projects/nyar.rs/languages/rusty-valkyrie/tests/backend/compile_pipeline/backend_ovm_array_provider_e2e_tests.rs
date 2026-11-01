#![cfg(feature = "ovm-lane")]
//! `std.collection.array` 形制的合同 → `[host_provider]` → `nyar.host` 宿主导入 →
//! OdinVM 执行的端到端正反合同。
//!
//! 正向：合同 `new`/`length`/`get`/`set` 经 adaptor 的 `[ovm("array.*")]` provider
//! 降低为 `CallImport`，加载期解析为 `HostOp`，执行结果正确。两种形制各自覆盖：
//! `[host_provider]` 包装体（体内调 `[ovm]` 无体 micro）与 `[ovm]` 直挂无体 provider。
//! 反向：合同有已解析调用但没有绑定 provider 时，链路必须结构化失败，
//! 不得空体成功或回落到任何旁路。
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use odin_bytecode::{OdinExportKind, encode_module};
use odin_vm::{executor::Executor, module::LoadedModule, value::Value};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const CONTRACTS: &str = r#"
namespace demo.collection.array;

[primitive("std::primitive::usize")]
structure usize { }

[primitive("std::primitive::i32")]
structure i32 { }

[host_contract]
micro new<T>(length: usize): [T]

[host_contract]
micro length<T>(array: [T]): usize

[host_contract]
micro get<T>(array: [T], index: usize): T

[host_contract]
micro set<T>(array: [T], index: usize, value: T): unit
"#;

// 与 std adaptor 正式形制一致：`[host_provider]` 包装体调用无体 `[ovm]` 宿主导入 micro。
// 泛型 provider 的具体代入由合同调用点在单态化播种（`host_provider_instantiation_seeds`）
// 中铸造，绑定按同签名闭合实例逐对配对重映射；provider 体内的 `[ovm]` 抽象调用随同一
// 固定点闭合并登记 external_call 合同，链接 BFS 与后端编码走同一条正式实例化路径。
const PROVIDERS: &str = r#"
namespace demo.adaptor.collection;

[host_provider(demo::collection::array::new)]
private micro array_host_new<T>(length: usize): [T] {
    return __array_ovm_new::<T>(length)
}

[host_provider(demo::collection::array::length)]
private micro array_host_length<T>(array: [T]): usize {
    return __array_ovm_length::<T>(array)
}

[host_provider(demo::collection::array::get)]
private micro array_host_get<T>(array: [T], index: usize): T {
    return __array_ovm_get::<T>(array, index)
}

[host_provider(demo::collection::array::set)]
private micro array_host_set<T>(array: [T], index: usize, value: T): unit {
    __array_ovm_set::<T>(array, index, value)
}

[ovm("array.new"), pure]
private micro __array_ovm_new<T>(length: usize): [T] { }

[ovm("array.length"), pure]
private micro __array_ovm_length<T>(array: [T]): usize { }

[ovm("array.get"), pure]
private micro __array_ovm_get<T>(array: [T], index: usize): T { }

[ovm("array.set")]
private micro __array_ovm_set<T>(array: [T], index: usize, value: T): unit { }
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

micro sized() -> usize {
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
) -> Result<odin_bytecode::OdinModuleData, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, bindings, "native")
        .map_err(|error| error.to_string())?;
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())
}

fn run_export(module: &odin_bytecode::OdinModuleData, symbol_suffix: &str) -> Value {
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
    let module = lower(&source_groups(true), &provider_bindings()).expect("合同与 provider 齐备时必须完成降低");
    assert_eq!(run_export(&module, "main"), Value::I32(42), "new→set→get 必须读回写入的元素");
    assert_eq!(run_export(&module, "sized"), Value::I32(5), "new→length 必须返回分配长度");
}

// 形制 2：`[ovm]` 直接挂在无体 `[host_provider]` micro 上（无包装体）。
// provider 声明本身即宿主导入；播种铸造其闭合抽象副本，contracts 收集为 ovm
// 导入合同，绑定把闭合合同实例配到该副本并重映射，消费模块持导入合同编码。
const PROVIDERS_OVM_DIRECT: &str = r#"
namespace demo.adaptor.collection;

[host_provider(demo::collection::array::new), ovm("array.new"), pure]
private micro array_host_new<T>(length: usize): [T] { }

[host_provider(demo::collection::array::length), ovm("array.length"), pure]
private micro array_host_length<T>(array: [T]): usize { }

[host_provider(demo::collection::array::get), ovm("array.get"), pure]
private micro array_host_get<T>(array: [T], index: usize): T { }

[host_provider(demo::collection::array::set), ovm("array.set")]
private micro array_host_set<T>(array: [T], index: usize, value: T): unit { }
"#;

#[test]
fn array_contract_ovm_direct_provider_form_executes_in_ovm() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", CONTRACTS),
        CompilerSourceGroup::single_source("adaptor", "adaptor", PROVIDERS_OVM_DIRECT).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let module = lower(&groups, &provider_bindings()).expect("形制 2（`[ovm]` 直挂无体 provider）必须完成降低");
    assert_eq!(run_export(&module, "main"), Value::I32(42), "形制 2: new→set→get 必须读回写入的元素");
    assert_eq!(run_export(&module, "sized"), Value::I32(5), "形制 2: new→length 必须返回分配长度");
}

#[test]
fn array_contract_without_provider_fails_structurally() {
    let error = lower(&source_groups(false), &[]).expect_err("缺 provider 的合同调用必须结构化失败，不得空体成功");
    assert!(error.contains("demo") && error.contains("array"), "错误必须指向未绑定 provider 的合同: {error}");
}
