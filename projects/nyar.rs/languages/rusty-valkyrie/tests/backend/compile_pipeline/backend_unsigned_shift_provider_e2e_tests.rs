//! `u32` / `u64` / `usize` 的 `>>` 合同 → `[wasm_operator]` provider → wasm 指令的
//! 端到端正反合同。
//!
//! 正向：无符号右移必须落到 `i32.shr_u`（0x76）/ `i64.shr_u`（0x88），真跑 Node
//! 断言逐值精确；`i32.rotl`（0x77）不得出现在无符号右移的代码里。
//! 反向：未知助记符必须结构化失败，不得静默降级成别的指令。

use crate::primitives::with_primitives;
use acorn_wasm::WasmBinaryModule;
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan, testing::lower_fragment_to_wasm_mir_module};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};
use std::{fs, process::Command};

const I32_SHR_U: u8 = 0x76;
const I32_ROTL: u8 = 0x77;
const I64_SHR_U: u8 = 0x88;

fn compile_to_wasm(source: &str, entry: &str) -> Result<WasmBinaryModule, String> {
    let groups = vec![CompilerSourceGroup::single_source("app", "demo", &with_primitives(source)).as_entry()];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
        .map_err(|error| error.to_string())?;
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), entry).map_err(|error| error.to_string())
}

fn code_section_bytes(module: &WasmBinaryModule) -> &[u8] {
    &module.sections.iter().find(|section| section.id == 10).expect("wasm 模块必须有 code 段").bytes
}

/// 在 Node 里实例化模块并逐个调用入口，调用表达式由调用方生成，
/// 结果统一按 `String(...)` 输出，`i64` 返回 `BigInt` 也保持十进制精确。
fn execute_cases(module: &WasmBinaryModule, calls: &[String]) -> Vec<String> {
    let bytes = module.to_bytes().expect("encode wasm module");
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("shift.wasm");
    let script_path = dir.path().join("run.mjs");
    fs::write(&wasm_path, &bytes).expect("write wasm");
    let script = format!(
        r#"
import {{ readFileSync }} from "node:fs";
const buf = readFileSync(process.argv[2]);
const mod = await WebAssembly.compile(buf);
const importObject = {{}};
for (const imp of WebAssembly.Module.imports(mod)) {{
  if (!importObject[imp.module]) importObject[imp.module] = {{}};
  if (imp.kind === "function") {{
    importObject[imp.module][imp.name] = () => 0;
  }} else if (imp.kind === "global") {{
    importObject[imp.module][imp.name] = new WebAssembly.Global({{ value: "i32", mutable: true }}, 0);
  }} else if (imp.kind === "memory") {{
    importObject[imp.module][imp.name] = new WebAssembly.Memory({{ initial: 1 }});
  }} else if (imp.kind === "table") {{
    importObject[imp.module][imp.name] = new WebAssembly.Table({{ initial: 0, element: "anyfunc" }});
  }}
}}
const inst = await WebAssembly.instantiate(mod, importObject);
const fn = inst.exports.main;
if (typeof fn !== "function") {{
  console.error("missing main export; have=" + Object.keys(inst.exports).join(","));
  process.exit(2);
}}
const results = [{}].map((call) => String(fn(...call)));
process.stdout.write(JSON.stringify(results));
"#,
        calls.join(", ")
    );
    fs::write(&script_path, script).expect("write script");
    let output = Command::new("node").arg(&script_path).arg(&wasm_path).current_dir(dir.path()).output().expect("spawn node to execute wasm");
    assert!(
        output.status.success(),
        "node wasm execute failed status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(String::from_utf8_lossy(&output.stdout).trim()).expect("node 结果必须是 JSON 数组")
}

const U32_SOURCE: &str = r#"
imply u32 {
    infix `>>`(self, rhs: Self): Self {
        u32_shr(self, rhs)
    }
}

[host_contract]
[wasm_operator("i32.shr_u")]
micro u32_shr(a: u32, b: u32): u32

[main]
micro main(a: u32, b: u32): u32 {
    return a >> b
}
"#;

const U64_SOURCE: &str = r#"
imply u64 {
    infix `>>`(self, rhs: Self): Self {
        u64_shr(self, rhs)
    }
}

[host_contract]
[wasm_operator("i64.shr_u")]
micro u64_shr(a: u64, b: u64): u64

[main]
micro main(a: u64, b: u64): u64 {
    return a >> b
}
"#;

const USIZE_SOURCE: &str = r#"
type usize = u32;

imply u32 {
    infix `>>`(self, rhs: Self): Self {
        u32_shr(self, rhs)
    }
}

[host_contract]
[wasm_operator("i32.shr_u")]
micro u32_shr(a: u32, b: u32): u32

[main]
micro main(a: usize, b: usize): usize {
    return a >> b
}
"#;

const I32_SIGNED_SOURCE: &str = r#"
imply i32 {
    infix `>>`(self, rhs: Self): Self {
        i32_shr(self, rhs)
    }
}

[host_contract]
[wasm_operator("i32.shr_s")]
micro i32_shr(a: i32, b: i32): i32

[main]
micro main(a: i32, b: i32): i32 {
    return a >> b
}
"#;

const I64_SIGNED_SOURCE: &str = r#"
imply i64 {
    infix `>>`(self, rhs: Self): Self {
        i64_shr(self, rhs)
    }
}

[host_contract]
[wasm_operator("i64.shr_s")]
micro i64_shr(a: i64, b: i64): i64

[main]
micro main(a: i64, b: i64): i64 {
    return a >> b
}
"#;

#[test]
fn u32_unsigned_shift_executes_exactly() {
    let module = compile_to_wasm(U32_SOURCE, "main").expect("u32 右移 provider 链必须完成降低");
    let code = code_section_bytes(&module);
    assert!(code.contains(&I32_SHR_U), "u32 `>>` 必须发射 `i32.shr_u`（0x76）");
    assert!(!code.contains(&I32_ROTL), "u32 `>>` 不得发射 `i32.rotl`（0x77）");
    let calls = vec![
        "[0x12345678, 3]".to_string(),
        "[0x7FFFFFFF, 31]".to_string(),
        "[0x80000000, 31]".to_string(),
        "[0, 31]".to_string(),
        "[5, 0]".to_string(),
        "[5, 1]".to_string(),
        "[4, 1]".to_string(),
    ];
    let actual = execute_cases(&module, &calls);
    assert_eq!(
        actual,
        vec!["38177487", "0", "1", "0", "5", "2", "2"],
        "0x12345678>>3=0x02468ACF、0x7FFFFFFF>>31=0、0x80000000>>31=1、0>>31=0、5>>0=5、5>>1=2、4>>1=2"
    );
}

#[test]
fn u64_unsigned_shift_executes_exactly() {
    let module = compile_to_wasm(U64_SOURCE, "main").expect("u64 右移 provider 链必须完成降低");
    let code = code_section_bytes(&module);
    assert!(code.contains(&I64_SHR_U), "u64 `>>` 必须发射 `i64.shr_u`（0x88）");
    let calls = vec!["[0xFEDCBA9876543210n, 4n]".to_string(), "[0xFEDCBA9876543210n, 63n]".to_string()];
    let actual = execute_cases(&module, &calls);
    assert_eq!(actual, vec!["1147797409030816545", "1"], "0xFEDCBA9876543210>>4=0x0FEDCBA987654321、>>63=1");
}

#[test]
fn usize_alias_unsigned_shift_executes_exactly() {
    let module = compile_to_wasm(USIZE_SOURCE, "main").expect("usize 右移 provider 链必须完成降低");
    let code = code_section_bytes(&module);
    assert!(code.contains(&I32_SHR_U), "wasm32 的 usize 别名必须走 `i32.shr_u`（0x76）");
    assert!(!code.contains(&I32_ROTL), "usize `>>` 不得发射 `i32.rotl`（0x77）");
    let calls = vec!["[0x12345678, 3]".to_string(), "[0x80000000, 31]".to_string()];
    let actual = execute_cases(&module, &calls);
    assert_eq!(actual, vec!["38177487", "1"], "usize 与 u32 同宽，右移结果必须一致");
}

#[test]
fn signed_shift_is_not_diverted() {
    let i32_module = compile_to_wasm(I32_SIGNED_SOURCE, "main").expect("i32 有符号右移必须完成降低");
    let actual = execute_cases(&i32_module, &["[-8, 1]".to_string(), "[-1, 31]".to_string()]);
    assert_eq!(actual, vec!["-4", "-1"], "`-8>>1=-4`、`-1>>31=-1` 必须保持算术右移");

    let i64_module = compile_to_wasm(I64_SIGNED_SOURCE, "main").expect("i64 有符号右移必须完成降低");
    let actual = execute_cases(&i64_module, &["[-8n, 1n]".to_string(), "[-1n, 63n]".to_string()]);
    assert_eq!(actual, vec!["-4", "-1"], "i64 算术右移不得被无符号修复带偏");
}

#[test]
fn unknown_unsigned_shift_mnemonic_fails_structurally() {
    let source = U32_SOURCE.replace("i32.shr_u", "i32.shr_z");
    let error = compile_to_wasm(&source, "main").expect_err("未知助记符必须结构化失败，不得静默降级");
    assert!(error.contains("i32.shr_z") && error.contains("可编码"), "错误必须指向未冻结的助记符: {error}");
}
