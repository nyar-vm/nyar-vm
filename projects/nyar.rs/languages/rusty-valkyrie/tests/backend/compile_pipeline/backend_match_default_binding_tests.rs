//! `default` 在 match 臂体表达式位与臂标记位的双重合同。
//!
//! `case None: default` 必须读取名为 `default` 的绑定（臂体内的普通标识符），
//! 而 `default:` 仍是合法的全匹配臂标记。两者不得互伤。wasm（Node 执行）与
//! OVM（Odin VM 执行）断言同一返回值。
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

micro three() -> i32 {
    return 3
}

micro unwrap_or_default(input: ProbeOption<i32>, default: i32) -> i32 {
    match input {
        case Some(value):
            value
        case None:
            default
    }
}

micro classify(input: ProbeOption<i32>) -> i32 {
    match input {
        case Some(value):
            value
        default:
            three()
    }
}

[main]
micro main() -> i32 {
    let a: i32 = unwrap_or_default(make_none(), 7)
    let b: ProbeOption<i32> = make_some(a)
    let c: i32 = classify(b)
    let d: i32 = classify(make_none())
    return unwrap_or_default(make_none(), d)
}
"#;

fn compile_app() -> nyar_types::CompiledProgram {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", PRIMITIVES),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
        .expect("`default` 标识符 fixture 必须完成编译")
}

fn plan_for(program: &nyar_types::CompiledProgram) -> TargetExecutionPlan {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    TargetExecutionPlan::from_compiled_program(program, &fragment, nyar::TheoryBundle::default(), &roots).expect("目标计划")
}

/// 在 Node（Wasm GC 可用）里实例化并读 `exports.main()` 的 i32 返回值。
fn execute_wasm_main_i32(bytes: &[u8]) -> i32 {
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("match_default_probe.wasm");
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

/// 链路编码：`unwrap_or_default(None, 7)` = 7（臂体读取 `default` 绑定），
/// `classify(Some(7))` = 7（Some 臂读载荷），`classify(None)` = 3（`default:` 臂生效），
/// 末次 `unwrap_or_default(None, 3)` = 3。任一环节断裂都会改变返回值或 trap。
#[test]
fn default_binding_in_arm_body_and_default_arm_both_work_wasm() {
    let program = compile_app();
    let plan = plan_for(&program);
    let bytes = lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .expect("wasm 模块必须完成降低")
        .to_bytes()
        .expect("wasm 必须完成编码");
    assert_eq!(execute_wasm_main_i32(&bytes), 3, "臂体 `default` 标识符与 `default:` 臂标记必须同时成立");
}

#[test]
fn default_binding_in_arm_body_and_default_arm_both_work_ovm() {
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
    assert_eq!(value, Value::I32(3), "OVM 必须与 wasm 返回同一值");
}
