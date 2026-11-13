//! nullary 变体 scrutinee 进入带 payload 的 match 臂：wasm 与 OVM 的真实执行合同。
//!
//! `SumPayloadGet` 必须在判别成功的后继块物化；判别块急切提取会让 wasm
//! `ref.cast` 在 `None` 的 null 载荷槽上 trap。本组把 fixture 分别编译到
//! wasm（Node 执行）与 OVM（Odin VM 执行），断言两侧返回同一正确值。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan, testing::lower_fragment_to_wasm_mir_module};
use odin_vm::{OdinVm, Value};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }
"#;

const APP: &str = r#"
namespace demo.app;

[tag(ProbeOptionKind)]
unite ProbeOption<T> {
    [tag(0)]
    [constructor]
    Some { value: T },
    [tag(1, default)]
    None,
}

micro make_none() -> ProbeOption<i32> {
    return None
}

micro make_some(value: i32) -> ProbeOption<i32> {
    return Some(value)
}

micro seven() -> i32 {
    return 7
}

micro unwrap_or_seven(input: ProbeOption<i32>) -> i32 {
    match input {
        case Some(value):
            value
        case None:
            seven()
    }
}

[main]
micro main() -> i32 {
    let first: i32 = unwrap_or_seven(make_none())
    let second: ProbeOption<i32> = make_some(first)
    return unwrap_or_seven(second)
}
"#;

fn compile_app() -> nyar_types::CompiledProgram {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", PRIMITIVES),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
        .expect("match 载荷延迟 fixture 必须完成编译")
}

fn plan_for(program: &nyar_types::CompiledProgram) -> TargetExecutionPlan {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    TargetExecutionPlan::from_compiled_program(program, &fragment, nyar::TheoryBundle::default(), &roots).expect("目标计划")
}

/// 在 Node（Wasm GC 可用）里实例化并读 `exports.main()` 的 i32 返回值。
fn execute_wasm_main_i32(bytes: &[u8]) -> i32 {
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("match_payload_probe.wasm");
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

#[test]
fn nullary_variant_match_arm_wasm_executes_without_trap() {
    let program = compile_app();
    let plan = plan_for(&program);
    let bytes = lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .expect("wasm 模块必须完成降低")
        .to_bytes()
        .expect("wasm 必须完成编码");
    assert_eq!(execute_wasm_main_i32(&bytes), 7, "None 进入带 payload 的 match 臂不得在 wasm 上 trap");
}

#[test]
fn nullary_variant_match_arm_ovm_executes_same_value() {
    let program = compile_app();
    let plan = plan_for(&program);
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .expect("ovm 模块必须完成降低");
    let entry =
        module.exports.iter().map(|export| export.symbol_name.clone()).find(|name| name.ends_with("main")).expect("必须导出 main 入口");
    let bytes = acorn_odin::encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("模块必须通过加载期校验");
    let value = vm.run(&loaded, &entry, Vec::new()).expect("main 必须在 OVM 内真实执行成功");
    assert_eq!(value, Value::I32(7), "OVM 必须与 wasm 返回同一值");
}
