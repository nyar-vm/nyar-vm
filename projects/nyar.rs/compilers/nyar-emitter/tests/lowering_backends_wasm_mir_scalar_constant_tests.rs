//! wasm 标量常量按目标槽 valtype 发射的合同：`i32.const` / `i64.const` 由槽决定，
//! 不得把 `i64` 字面量经 `as i32` 截断后 sign-extend 回 `i64`。
//!
//! 覆盖面：
//! - i32 槽正例、i64 槽正例（含 `i64::MAX`）真跑 Node 取值；
//! - 常量作为静态调用实参进入 `i64` 形参槽（历史上走无数值语义的兜底分支）；
//! - i32 槽收到超出 i32 表示范围的字面量必须结构化失败，禁止静默截断。

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
    nyar_backend_wasi::WasmBinaryModule,
};
use nyar_types::{ItemInstanceId, NyarType};
use std::{collections::BTreeMap, process::Command};

const I32: NyarType = NyarType::Integer32 { signed: true };
const I64: NyarType = NyarType::Integer64 { signed: true };

fn empty_function(symbol: &str, param_types: Vec<NyarType>, return_type: NyarType, blocks: Vec<Block>) -> ExecutableFunction {
    let entry = &blocks[0];
    let mut value_types = BTreeMap::new();
    for parameter in entry.parameters.iter().zip(&param_types) {
        value_types.insert(*parameter.0, parameter.1.clone());
    }
    ExecutableFunction {
        symbol: symbol.to_string(),
        return_type,
        param_types,
        value_types,
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks,
        diagnostics: Vec::new(),
    }
}

/// `main() -> ty`：结果槽直接载入整型字面量。
fn constant_function(symbol: &str, ty: NyarType, value: i64) -> ExecutableFunction {
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(value), ty: Some(ty.clone()) });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions: vec![Instruction { results: vec![ValueRef(0)], ..load }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    };
    let mut function = empty_function(symbol, Vec::new(), ty.clone(), vec![block]);
    function.value_types.insert(ValueRef(0), ty);
    function
}

/// `echo(a: i64) -> i64`：恒等函数，用作静态调用目标。
fn identity_i64(symbol: &str) -> ExecutableFunction {
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0)],
        instructions: Vec::new(),
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    };
    empty_function(symbol, vec![I64.clone()], I64.clone(), vec![block])
}

/// `main() -> i64`：把整型字面量作为 `i64` 形参传给 `echo`，返回调用结果。
fn call_with_constant(symbol: &str, callee: ItemInstanceId, value: i64) -> ExecutableFunction {
    let call = Instruction::from_kind(InstructionKind::Call { callee: Operand::Item(callee), arguments: vec![Operand::Constant(Constant::Int(value))] });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions: vec![Instruction { results: vec![ValueRef(0)], ..call }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    };
    let mut function = empty_function(symbol, Vec::new(), I64.clone(), vec![block]);
    function.value_types.insert(ValueRef(0), I64.clone());
    function
}

fn lower(functions: Vec<(&str, ExecutableFunction)>, entry: &str) -> nyar_emitter::Result<WasmBinaryModule> {
    let names = functions.iter().map(|(name, _)| QualifiedName::new(vec![Identifier::new(*name)])).collect::<Vec<_>>();
    let map: BTreeMap<QualifiedName, ExecutableFunction> =
        names.into_iter().zip(functions.into_iter().map(|(_, function)| function)).collect();
    let entry_name = QualifiedName::new(vec![Identifier::new(entry)]);
    let mut plan = TargetExecutionPlan::from_functions(map);
    plan.testing_module_name("scalar_constant").testing_entry_operation(entry_name);
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, entry)
}

/// Node 实例化并调用 `main`，把返回值按 JS 语义转成字符串（i32 → Number，i64 → BigInt）。
fn execute_main(module: &WasmBinaryModule) -> String {
    let bytes = module.to_bytes().expect("encode wasm module");
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("scalar_constant.wasm");
    let script_path = dir.path().join("run.mjs");
    std::fs::write(&wasm_path, &bytes).expect("write wasm");
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
  console.error("missing main/run export; have=" + Object.keys(inst.exports).join(","));
  process.exit(2);
}
process.stdout.write(String(fn()));
"#,
    )
    .expect("write script");
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .current_dir(dir.path())
        .output()
        .expect("spawn node to execute wasm");
    assert!(
        output.status.success(),
        "node wasm execute failed status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn i32_slot_constant_executes_exactly() {
    let module = lower(vec![("main", constant_function("main", I32.clone(), 7))], "main").expect("i32 常量必须完成 lowering");
    assert_eq!(execute_main(&module), "7", "i32 槽常量必须按 i32.const 发射");
}

#[test]
fn i64_slot_constant_executes_exactly() {
    let module = lower(vec![("main", constant_function("main", I64.clone(), 2_147_483_648))], "main").expect("i64 常量必须完成 lowering");
    assert_eq!(execute_main(&module), "2147483648", "i64 槽常量必须按 i64.const 发射，不得截断到 i32");
}

#[test]
fn i64_slot_max_constant_executes_exactly() {
    let module = lower(vec![("main", constant_function("main", I64.clone(), i64::MAX))], "main").expect("i64::MAX 常量必须完成 lowering");
    assert_eq!(execute_main(&module), i64::MAX.to_string(), "i64::MAX 必须逐位保真");
}

#[test]
fn constant_argument_to_i64_parameter_executes_exactly() {
    let callee = ItemInstanceId::from_index(0).expect("echo 实例身份");
    let module = lower(vec![("echo", identity_i64("echo")), ("main", call_with_constant("main", callee, i64::MAX))], "main")
        .expect("常量实参必须完成 lowering");
    assert_eq!(execute_main(&module), i64::MAX.to_string(), "字面量实参进入 i64 形参槽必须保真，不得先截断成 i32 再符号扩展");
}

#[test]
fn i32_slot_constant_out_of_range_fails_structured() {
    let error = lower(vec![("main", constant_function("main", I32.clone(), 2_147_483_648))], "main")
        .expect_err("超出 i32 表示范围的字面量不得静默截断");
    assert!(error.contains("2147483648"), "失败信息必须带出事常量值: {error}");
    assert!(error.contains("i32"), "失败信息必须说明目标槽位宽: {error}");
}
