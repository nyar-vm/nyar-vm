//! OVM 导出面判据回归：导出只来自**声明的导出面**（`[export]` 冻结名 ∪ 程序入口），
//! 与 wasm 的 `export_names` 同源；可达闭包内带函数类型形参的内部辅助函数
//! 不得被当成库导出根，而真正声明为 `[export]` 的函数类型成员仍失败闭合并点名成员。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use odin_vm::{OdinVm, Value};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const PRIMITIVES: &str = r#"namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }"#;

fn source_groups(app: &str) -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", PRIMITIVES),
        CompilerSourceGroup::single_source("app", "demo", app).as_entry().with_direct_dependencies(vec!["core".into()]),
    ]
}

fn backend_plan(groups: &[CompilerSourceGroup]) -> Result<TargetExecutionPlan, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, &[], &[], "native")
        .map_err(|error| error.to_string())?;
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())
}

fn lower(plan: TargetExecutionPlan) -> Result<acorn_odin::OdinModuleData, String> {
    nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())
}

fn export_names(module: &acorn_odin::OdinModuleData) -> Vec<String> {
    module.exports.iter().map(|export| export.symbol_name.clone()).collect()
}

fn run_export(module: &acorn_odin::OdinModuleData, name: &str) -> Value {
    let bytes = acorn_odin::encode_module(module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("编码后的模块必须可加载");
    vm.run(&loaded, name, Vec::new()).expect("导出入口必须在内置 OVM 运行时执行成功")
}

/// 内部辅助函数带函数类型形参时不进入导出面：导入集合只含程序入口，模块仍能降低并执行。
#[test]
fn internal_function_typed_helper_stays_out_of_the_export_surface() {
    let app = r#"namespace demo;

micro apply(f: micro(i32) -> i32) -> i32 {
    return f(7)
}

[main]
micro main() -> i32 {
    return apply(micro(value: i32) -> i32 { return value })
}"#;
    let plan = backend_plan(&source_groups(app)).expect("内部函数类型形参不得阻断目标计划");
    assert!(plan.export_names().is_empty(), "无 `[export]` 时不得有公共导出名");
    assert!(plan.entry_operation().is_some(), "程序入口必须由独立字段登记");
    let module = lower(plan).expect("内部函数类型形参不得阻断 OVM 降低");
    let names = export_names(&module);
    assert!(names.iter().all(|name| !name.ends_with("apply")), "内部辅助函数不得进导出表: {names:?}");
    assert_eq!(names.len(), 1, "导出面只含程序入口: {names:?}");
    assert_eq!(run_export(&module, &names[0]), Value::I32(7), "间接调用必须执行成功");
}

/// 真正声明为 `[export]` 的函数类型成员继续失败闭合，错误必须点名该成员。
#[test]
fn declared_function_typed_export_fails_closed_naming_the_member() {
    let app = r#"namespace demo;

[export(name: "apply")]
micro apply(f: micro(i32) -> i32) -> i32 {
    return f(1)
}

[main]
micro main() -> i32 {
    return apply(micro(value: i32) -> i32 { return value })
}"#;
    // 公共目标计划只登记逻辑导出身份与签名，不再先套用 Wasm ABI 资格规则。
    let plan = backend_plan(&source_groups(app)).expect("公共计划不得替目标决定导出载体资格");
    let error = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module_with_abi_for_testing(
        &FragmentSubmission::from_backend_plan(plan),
    )
    .expect_err("函数类型形参在 OVM 导出 ABI 边界必须失败闭合");
    assert!(error.contains("apply"), "错误必须点名导出成员: {error}");
    assert!(error.contains("函数类型") || error.contains("库导出"), "错误必须说明函数类型不可过边界: {error}");
}

/// OVM 导出集合必须与 wasm 的声明导出名同源：公共冻结名原样进入导出表，无额外可达成员。
#[test]
fn ovm_library_exports_match_the_shared_declared_surface() {
    let app = r#"namespace demo;

[export(name: "first")]
micro first(value: i32) -> i32 { return value }

[export(name: "second")]
micro second(value: i32) -> i32 { return value }"#;
    let plan = backend_plan(&source_groups(app)).expect("两个标量导出必须冻结 ABI");
    let mut declared = plan.export_names().values().cloned().collect::<Vec<_>>();
    if let Some(entry) = plan.entry_operation() {
        declared.push(plan.abi_name_for_instance(entry).expect("入口必须有 ABI 名").to_string());
    }
    declared.sort();
    assert_eq!(declared, vec!["first".to_owned(), "second".to_owned()], "公共冻结名必须原样");
    let module = lower(plan).expect("标量导出必须完成降低");
    let mut names = export_names(&module);
    names.sort();
    assert_eq!(names, declared, "OVM 导出集合必须与 wasm 声明导出名同源");
}
