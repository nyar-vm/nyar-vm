//! 字符/字节字面量比较的四 lane 端到端合同。
//!
//! 锁定事实：`'x'`（char）与 `b'x'`（u8）字面量经形参特化（char 形参直绑、数值形参
//! 按码位/字节值绑定）进入已解析调用后，wasm/ovm/jvm/clr 四 lane 都按同一整数常量
//! 编码；wasm 经 Node 真实执行、ovm 经内置 Odin VM 真实执行，main 返回 0。
//! 缺 provider 的变体必须在编译边界结构化失败，不得空体成功。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan, testing::lower_fragment_to_wasm_mir_module};
use odin_vm::{OdinVm, Value};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler, compiler::emitter::pipeline::CompilerHostProviderBinding};

/// 最小 primitive 闭包：bool 的 `!` 是纯 V 实现，不需要宿主 provider。
const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::u32")]
structure u32 { }

namespace std.text;

[primitive("std::text::char")]
structure char { }

namespace demo.primitive;

imply bool {
    prefix `!`(self): bool {
        if self {
            return false
        }
        return true
    }
}
"#;

/// 等值合同：无体，实现来源只能是 Resolver 选定的 provider。
///
/// 只覆盖 `u32` 与 `char` 两种形参：`u8`/`u16` 在 wasm/ovm 上没有目标载体
/// （VM 无掩码指令，std.adaptor 刻意不提供 provider），字节字面量的 `u8` 默认归属
/// 由 frontend/MIR 层测试承担，这里验证字节字面量对 `u32` 形参的特化。
const CONTRACTS: &str = r#"
namespace demo.primitive.literal_ops;

[host_contract]
micro eq_u32(a: u32, b: u32): bool

[host_contract]
micro eq_char(a: char, b: char): bool
"#;

/// 应用：字符/字节字面量在 char 与 u32 形参上的直接比较，不写 `as`。
const APP: &str = r#"
namespace demo.app;

[main]
micro main() -> i32 {
    let space: char = ' '
    let tab: char = '\t'

    if !eq_char(space, ' ') { return 101 }
    if !eq_char(tab, '\t') { return 102 }
    if eq_char(space, 'x') { return 103 }
    if !eq_u32(32, ' ') { return 104 }
    if !eq_u32(32, b' ') { return 105 }
    if eq_u32(32, b'x') { return 106 }
    return 0
}
"#;

fn providers_for(lane_annotation: &str) -> String {
    format!(
        r#"
namespace demo.adaptor.literal_ops;

[host_provider(demo::primitive::literal_ops::eq_u32)]
[{lane_annotation}]
micro eq_u32(a: u32, b: u32): bool

[host_provider(demo::primitive::literal_ops::eq_char)]
[{lane_annotation}]
micro eq_char(a: char, b: char): bool
"#
    )
}

fn provider_bindings() -> Vec<CompilerHostProviderBinding> {
    use rusty_valkyrie::compiler::semantic::closure::micro_declaration_key;
    let contract_ns = ["demo".to_owned(), "primitive".to_owned(), "literal_ops".to_owned()];
    let provider_ns = ["demo".to_owned(), "adaptor".to_owned(), "literal_ops".to_owned()];
    ["eq_u32", "eq_char"]
        .into_iter()
        .map(|name| CompilerHostProviderBinding {
            contract: micro_declaration_key("core", "core.v", &contract_ns, name),
            provider: micro_declaration_key("adaptor", "adaptor.v", &provider_ns, name),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        })
        .collect()
}

fn groups_for(providers: &str) -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", &format!("{PRIMITIVES}\n{CONTRACTS}")),
        CompilerSourceGroup::single_source("adaptor", "adaptor", providers).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ]
}

fn compile(groups: &[CompilerSourceGroup], bindings: &[CompilerHostProviderBinding]) -> Result<nyar_types::CompiledProgram, String> {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, bindings, &[], "native")
        .map_err(|error| error.to_string())
}

fn plan_for(program: &nyar_types::CompiledProgram) -> Result<TargetExecutionPlan, String> {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    TargetExecutionPlan::from_compiled_program(program, &fragment, nyar::TheoryBundle::default(), &roots).map_err(|error| error.to_string())
}

fn lower_wasm_bytes(groups: &[CompilerSourceGroup], bindings: &[CompilerHostProviderBinding]) -> Result<Vec<u8>, String> {
    let program = compile(groups, bindings)?;
    let plan = plan_for(&program)?;
    lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .map_err(|error| error.to_string())?
        .to_bytes()
        .map_err(|error| error.to_string())
}

/// 在 Node（Wasm GC 可用）里实例化并读 `exports.main()` 的 i32 返回值。
fn execute_wasm_main_i32(bytes: &[u8]) -> i32 {
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("literal_probe.wasm");
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
fn single_quote_literals_wasm_execute_end_to_end() {
    let bytes = lower_wasm_bytes(&groups_for(&providers_for("wasm_operator(\"i32.eq\")")), &provider_bindings())
        .expect("wasm 字面量等值链必须完成降低");
    assert!(bytes.contains(&0x46), "i32.eq 必须落到冻结 opcode 字节");
    assert_eq!(execute_wasm_main_i32(&bytes), 0, "全部字面量比较必须在 Node 内真实执行为真");
}

#[test]
fn single_quote_literals_ovm_execute_end_to_end() {
    let groups = groups_for(&providers_for("ovm_operator(\"i32.eq\")"));
    let program = compile(&groups, &provider_bindings()).expect("ovm 字面量等值链必须完成编译");
    let plan = plan_for(&program).expect("ovm 目标计划");
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .expect("ovm 模块必须完成降低");
    let entry = module.exports.iter().map(|export| export.symbol_name.clone()).find(|name| name.ends_with("main")).expect("必须导出 main 入口");
    let bytes = acorn_odin::encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("编码后的模块必须可加载");
    let value = vm.run(&loaded, &entry, Vec::new()).expect("main 必须在内置 OVM 运行时执行成功");
    assert_eq!(value, Value::I32(0), "全部字面量比较必须在 OVM 内真实执行为真");
}

/// 反向：缺 provider 的字面量合同调用必须在编译边界结构化失败。
#[test]
fn single_quote_literal_contract_without_provider_fails_closed() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", &format!("{PRIMITIVES}\n{CONTRACTS}")),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let message = compile(&groups, &[]).expect_err("缺 provider 的合同调用必须结构化失败，不得空体成功");
    assert!(message.contains("eq_"), "错误必须指向未绑定 provider 的合同: {message}");
}

/// JVM lane：`integer.compare_equal` 配方必须把字符/字节字面量比较编码进 class 主方法。
/// 编码级证据（配方冻结门在计划构造时已校验载体）；本机无 JVM 运行时，不伪造执行。
#[test]
fn single_quote_literals_jvm_encode() {
    let groups = groups_for(&providers_for("jvm_operator(\"integer.compare_equal\")"));
    let program = compile(&groups, &provider_bindings()).expect("jvm 字面量等值链必须完成编译");
    let plan = plan_for(&program).expect("jvm 目标计划必须冻结 operator 载体");
    let class =
        nyar_emitter::testing::lower_fragment_to_jvm_class(&FragmentSubmission::from_backend_plan(plan)).expect("jvm class 必须完成编码");
    let main = class.methods.iter().find(|method| method.name.contains("main")).expect("class 必须有 main 方法");
    let code = main.code.as_ref().expect("main 必须有方法体");
    assert!(!code.instructions.is_empty(), "main 方法必须有编码指令");
}

/// CLR lane：`i32.eq` 配方必须把字符/字节字面量比较编码进 MSIL 模块。
/// 编码级证据；本机无 CLR 运行时，不伪造执行。
#[test]
fn single_quote_literals_clr_encode() {
    let groups = groups_for(&providers_for("clr_operator(\"i32.eq\")"));
    let program = compile(&groups, &provider_bindings()).expect("clr 字面量等值链必须完成编译");
    let plan = plan_for(&program).expect("clr 目标计划必须冻结 operator 载体");
    let module =
        nyar_emitter::testing::lower_fragment_to_clr_module(&FragmentSubmission::from_backend_plan(plan)).expect("clr MSIL 模块必须完成编码");
    assert!(!module.global_methods.is_empty(), "MSIL 模块必须有方法体");
}
