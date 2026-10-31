//! 具名函数作值的 Wasm 编码合同：`LoadFunctionRef` 降为 funcref table 槽位常量，
//! `Invoke{callee: Value}` 降为类型化 `call_indirect`。
//!
//! 验收面：happy path 模块含 table(4)/elem(9) 段且 Node 实际执行间接调用得到结果；
//! fail-closed：引用实例没有函数体、callee 缺少冻结 Function 签名、签名未注册
//! functype 三种缺事实形状都在编码前结构化失败，禁止猜测救活。

use acorn_wasm::{WasmBinaryModule, WasmOpcode};
use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use nyar_types::{ItemInstanceId, NyarFunctionType, NyarType};
use std::{collections::BTreeMap, process::Command};

const I32: NyarType = NyarType::Integer32 { signed: true };

fn function_type(params: Vec<NyarType>, return_type: NyarType) -> NyarType {
    NyarType::Function(Box::new(NyarFunctionType { params, return_type }))
}

/// `cmp(a: i32, b: i32) -> i32`：忽略实参，恒返回 42（间接调用命中即证 table/elem/typeidx 全对）。
fn cmp_function() -> ExecutableFunction {
    let constant = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(42), ty: Some(I32) });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0), ValueRef(1)],
        instructions: vec![Instruction { results: vec![ValueRef(2)], ..constant }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(2))) },
    };
    ExecutableFunction {
        symbol: "cmp".to_string(),
        return_type: I32,
        param_types: vec![I32, I32],
        value_types: BTreeMap::from([(ValueRef(0), I32), (ValueRef(1), I32), (ValueRef(2), I32)]),
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
        blocks: vec![block],
        diagnostics: Vec::new(),
    }
}

/// `main()`：`LoadFunctionRef(cmp)` 物化函数值 → 间接调用 → 返回结果。
///
/// `callee_type` 是函数值 SSA 的登记类型；fail-closed 测试用它注入缺事实形状。
/// `referenced_instance` 是 `LoadFunctionRef` 引用的实例身份。
fn main_function(callee_type: NyarType, referenced_instance: ItemInstanceId) -> ExecutableFunction {
    let load = Instruction::from_kind(InstructionKind::LoadFunctionRef { instance: referenced_instance });
    let call = Instruction::from_kind(InstructionKind::Call {
        callee: Operand::Value(ValueRef(0)),
        arguments: vec![Operand::Constant(Constant::Int(3)), Operand::Constant(Constant::Int(5))],
    });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions: vec![
            Instruction { results: vec![ValueRef(0)], ..load },
            Instruction { results: vec![ValueRef(1)], ..call },
        ],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
    };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: I32,
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), callee_type), (ValueRef(1), I32)]),
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
        blocks: vec![block],
        diagnostics: Vec::new(),
    }
}

/// `from_functions` 按名称排序铸造实例：`cmp` → 0，`main` → 1。
fn cmp_instance() -> ItemInstanceId {
    ItemInstanceId::from_index(0).expect("测试实例身份")
}

fn lower(callee_type: NyarType, referenced_instance: ItemInstanceId) -> nyar_emitter::Result<WasmBinaryModule> {
    let cmp_name = QualifiedName::new(vec![Identifier::new("cmp")]);
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(cmp_name, cmp_function()), (main_name.clone(), main_function(callee_type, referenced_instance))]));
    plan.testing_module_name("function_value_exec").testing_entry_operation(main_name);
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main")
}

/// 用 Node（Wasm GC 可用）实例化并读取 `exports.main()`。
/// 字节码含 opcode 不等于可执行——间接调用必须真正跑通。
fn execute_main_i32(module: &WasmBinaryModule) -> i32 {
    let bytes = module.to_bytes().expect("encode wasm module");
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("function_value.wasm");
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
const value = fn();
if (typeof value !== "number") {
  console.error("export did not return number: " + typeof value);
  process.exit(3);
}
process.stdout.write(String(value | 0));
"#,
    )
    .expect("write script");
    let output = Command::new("node").arg(&script_path).arg(&wasm_path).output().expect("spawn node to execute wasm");
    if !output.status.success() {
        panic!(
            "node wasm execute failed status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    text.parse::<i32>().unwrap_or_else(|_| panic!("expected i32 stdout, got {text:?}"))
}

/// happy path：`LoadFunctionRef` + 间接调用降为可执行 wasm，Node 实际执行得到 42；
/// 模块必须携带 funcref table(4) 与 active elem(9) 段，code 段必须含 `call_indirect`。
#[test]
fn wasm_function_value_indirect_call_executes_and_carries_table() {
    let callee_ty = function_type(vec![I32, I32], I32);
    let module = lower(callee_ty, cmp_instance()).expect("函数值间接调用必须完成 lowering");
    let table = module.sections.iter().find(|section| section.id == 4).expect("被作值引用的函数必须产生 table 段");
    assert_eq!(table.bytes, vec![0x01, 0x70, 0x00, 0x01], "table 段必须是单个 funcref、min=1");
    let elem = module.sections.iter().find(|section| section.id == 9).expect("table 槽位必须由 active elem 段初始化");
    assert!(elem.bytes.contains(&0x00), "active elem 段 offset 表达式必须是 i32.const 0");
    let code = module.sections.iter().find(|section| section.id == 10).expect("code 段");
    assert!(code.bytes.contains(&WasmOpcode::CallIndirect.as_u8()), "间接调用必须发射 call_indirect");
    // section id 必须升序（table 在 function 与 memory 之间，elem 在 export 与 code 之间）。
    let ids: Vec<u8> = module.sections.iter().filter(|section| section.id != 0).map(|section| section.id).collect();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "标准段 id 必须严格升序: {ids:?}");
    assert_eq!(execute_main_i32(&module), 42, "间接调用必须真实执行到被引用的函数体");
}

/// fail-closed：`LoadFunctionRef` 引用没有函数体的实例属于计划事实缺失，
/// 编码前结构化失败，禁止现场补造 table 项。
#[test]
fn wasm_function_value_reference_without_body_fails_closed() {
    let orphan = ItemInstanceId::from_index(9).expect("测试实例身份");
    let error = lower(function_type(vec![I32, I32], I32), orphan).expect_err("未登记函数体不得静默编码");
    assert!(error.contains("函数值"), "错误必须是结构化的函数值事实缺失: {error}");
}

/// fail-closed：间接调用 callee 的 SSA 值缺少冻结 Function 签名时结构化失败，
/// 禁止按实参个数或候选顺序猜签名。
#[test]
fn wasm_indirect_call_without_frozen_signature_fails_closed() {
    let error = lower(I32, cmp_instance()).expect_err("callee 非 Function 类型不得静默编码");
    assert!(error.contains("Function 签名"), "错误必须是结构化的签名缺失: {error}");
}

/// fail-closed：callee 签名没有命中模块已注册的 functype 时结构化失败，
/// 禁止为间接调用新造类型条目。
#[test]
fn wasm_indirect_call_with_unregistered_signature_fails_closed() {
    let i64_ty = NyarType::Integer64 { signed: true };
    let error = lower(function_type(vec![i64_ty.clone(), i64_ty], I32), cmp_instance()).expect_err("未注册签名不得新造 typeidx");
    assert!(error.contains("functype"), "错误必须是结构化的类型注册缺失: {error}");
}
