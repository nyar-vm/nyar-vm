//! 多字段 unite 变体 payload 的值级端到端合同。
//!
//! 多字段 payload 在 canonical 里是变体记录（字段按声明顺序），目标计划必须
//! 为它登记布局身份。本组分别用按位置 `case Both(left, right)` 与按名
//! `case Both { right, left }` 解构，编译到 wasm（Node 执行）与 OVM（Odin VM
//! 执行），断言两侧取到的字段值与记录布局字段顺序/类型。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan, testing::lower_fragment_to_wasm_mir_module};
use nyar_types::NyarType;
use odin_vm::{OdinVm, Value};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }
"#;

const PAIR_DECLARATION: &str = r#"
namespace demo.app;

unite Pair {
    [constructor]
    Both { left: i32, right: i32 },
    Neither,
}
"#;

/// 按位置解构：`right` 必须是第二个声明字段。
const POSITION_APP: &str = r#"
namespace demo.app;

micro build(left: i32, right: i32) -> Pair {
    return Both { left: left, right: right }
}

micro by_position(input: Pair) -> i32 {
    match input {
        case Both(left, right):
            right
        case Neither:
            0
    }
}

[main]
micro main() -> i32 {
    let pair: Pair = build(4, 5)
    return by_position(pair)
}
"#;

/// 按名解构且模式内顺序反转：按位置实现会把 `right` 误绑到首字段 4，
/// 只有真正按声明字段名绑定才会取到 5。
const NAME_APP: &str = r#"
namespace demo.app;

micro build(left: i32, right: i32) -> Pair {
    return Both { left: left, right: right }
}

micro by_name(input: Pair) -> i32 {
    match input {
        case Both { right, left }:
            right
        case Neither:
            0
    }
}

[main]
micro main() -> i32 {
    let pair: Pair = build(4, 5)
    return by_name(pair)
}
"#;

fn compile_app(app: &str) -> nyar_types::CompiledProgram {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", PRIMITIVES),
        CompilerSourceGroup::single_source("app", "demo", &format!("{PAIR_DECLARATION}{app}"))
            .as_entry()
            .with_direct_dependencies(vec!["core".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
        .expect("多字段变体 fixture 必须完成编译")
}

fn plan_for(program: &nyar_types::CompiledProgram) -> TargetExecutionPlan {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    TargetExecutionPlan::from_compiled_program(program, &fragment, nyar::TheoryBundle::default(), &roots).expect("目标计划")
}

/// 多字段 payload 记录必须在目标计划里携带已登记布局，字段顺序按声明顺序。
fn assert_record_layout(program: &nyar_types::CompiledProgram) {
    let linked = &program.canonical().linked;
    let record = linked.nominal_instances.values().find(|record| record.fields.len() == 2).expect("多字段 payload 必须登记名义记录");
    assert_eq!(record.ty, record.declaration, "零实参变体记录的类型身份必须等于其声明身份");
    let layout_id = linked.aggregate_layout_by_type.get(&record.ty).copied().expect("多字段 payload 记录必须登记目标布局");
    let layout = linked.aggregate_layouts.layouts.iter().find(|layout| layout.id == layout_id).expect("布局 id 必须指向已登记布局");
    let names: Vec<&str> = layout.fields.iter().map(|field| field.name.as_str()).collect();
    assert_eq!(names, ["left", "right"], "记录字段布局必须保持声明顺序");
    let field_types: Vec<&NyarType> = layout.fields.iter().map(|field| &field.ty).collect();
    assert_eq!(field_types, [&NyarType::Integer32 { signed: true }, &NyarType::Integer32 { signed: true }], "记录字段布局必须保留字段类型");
}

/// 在 Node（Wasm GC 可用）里实例化并读 `exports.main()` 的 i32 返回值。
fn execute_wasm_main_i32(bytes: &[u8]) -> i32 {
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("multifield_variant_probe.wasm");
    let script_path = dir.path().join("run.mjs");
    std::fs::write(&wasm_path, bytes).expect("write wasm");
    std::fs::write(
        &script_path,
        r#"
import { readFileSync } from "node:fs";
const buf = readFileSync(process.argv[2]);
const mod = await WebAssembly.compile(buf);
const importObject = {};
for (const imp of WebAssembly.Module.imports(mod)) {
  if (!importObject[imp.module]) importObject[imp.module] = {};
  if (imp.kind === "function") {
    importObject[imp.module][imp.name] = () => 0;
  } else if (imp.kind === "global") {
    importObject[imp.module][imp.name] = new WebAssembly.Global({ value: "i32", mutable: true }, 0);
  } else if (imp.kind === "memory") {
    importObject[imp.module][imp.name] = new WebAssembly.Memory({ initial: 1 });
  } else if (imp.kind === "table") {
    importObject[imp.module][imp.name] = new WebAssembly.Table({ initial: 0, element: "anyfunc" });
  }
}
const inst = await WebAssembly.instantiate(mod, importObject);
const fn = inst.exports.main ?? inst.exports.run;
if (typeof fn !== "function") {
  console.error("missing main/run export");
  process.exit(2);
}
process.stdout.write(String(fn() | 0));
"#,
    )
    .expect("write script");
    let output = std::process::Command::new("node").arg(&script_path).arg(&wasm_path).output().expect("spawn node to execute wasm");
    if !output.status.success() {
        panic!("node wasm execute failed status={:?}\nstderr={}", output.status.code(), String::from_utf8_lossy(&output.stderr));
    }
    String::from_utf8_lossy(&output.stdout).trim().parse::<i32>().expect("wasm main 必须返回 i32")
}

fn run_ovm_main(program: &nyar_types::CompiledProgram) -> Value {
    let plan = plan_for(program);
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .expect("ovm 模块必须完成降低");
    let entry = module.exports.iter().map(|export| export.symbol_name.clone()).find(|name| name.ends_with("main")).expect("必须导出 main 入口");
    let bytes = acorn_odin::encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("模块必须通过加载期校验");
    vm.run(&loaded, &entry, Vec::new()).expect("main 必须在 OVM 内真实执行成功")
}

#[test]
fn multifield_payload_by_position_runs_on_both_targets() {
    let program = compile_app(POSITION_APP);
    assert_record_layout(&program);
    let plan = plan_for(&program);
    let bytes = lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .expect("wasm 模块必须完成降低")
        .to_bytes()
        .expect("wasm 必须完成编码");
    assert_eq!(execute_wasm_main_i32(&bytes), 5, "按位置解构必须取到第二个声明字段 `right`");
    assert_eq!(run_ovm_main(&program), Value::I32(5), "OVM 必须与 wasm 一致取到 `right`");
}

#[test]
fn multifield_payload_by_name_runs_on_both_targets() {
    let program = compile_app(NAME_APP);
    assert_record_layout(&program);
    let plan = plan_for(&program);
    let bytes = lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .expect("wasm 模块必须完成降低")
        .to_bytes()
        .expect("wasm 必须完成编码");
    assert_eq!(execute_wasm_main_i32(&bytes), 5, "按名解构写入顺序反转也必须按声明字段名取到 `right`");
    assert_eq!(run_ovm_main(&program), Value::I32(5), "OVM 必须与 wasm 一致按名取到 `right`");
}
