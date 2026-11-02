//! `Raise` effect 终止符的 wasm 目标合同：未捕获 raise 编码为 `unreachable` trap。
//!
//! 正面：含 `PerformEffect { Raise }` 的函数必须完成 lowering，产物含 `unreachable`，
//! 在 Node 里执行即 trap。零形参与单形参 resume 块（真实 `std::types::panic` 形态）
//! 都必须通过槽规划。

use acorn_wasm::{WasmBinaryModule, WasmOpcode};
use nyar::{Identifier, NyarType, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, EffectKind, ExecutableFunction, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use std::{collections::BTreeMap, process::Command};

/// `main() -> void`：入口块直接 `PerformEffect Raise`，resume 块带 `with_resume_param`
/// 指定的形参数并 `return`。
fn raising_function(with_resume_param: bool) -> ExecutableFunction {
    let resume_parameters = if with_resume_param { vec![ValueRef(0)] } else { Vec::new() };
    let value_types = if with_resume_param { BTreeMap::from([(ValueRef(0), NyarType::Unit)]) } else { BTreeMap::new() };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: NyarType::Unit,
        param_types: Vec::new(),
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
        blocks: vec![
            Block {
                id: BlockRef(0),
                label: "entry".to_string(),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::PerformEffect { effect: EffectKind::Raise, payload: None, resume_target: BlockRef(1) },
            },
            Block {
                id: BlockRef(1),
                label: "raise_resume".to_string(),
                parameters: resume_parameters,
                instructions: Vec::new(),
                terminator: Terminator::Return { value: None },
            },
        ],
        diagnostics: Vec::new(),
    }
}

fn lower_main(function: ExecutableFunction) -> WasmBinaryModule {
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), function)]));
    plan.testing_module_name("raise_trap").testing_entry_operation(main);
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main").expect("含 `Raise` 的函数必须完成 wasm lowering")
}

/// 在 Node（wasm-gc 可用）里实例化并调用 `main`，返回 `"trapped"` / `"returned"`。
fn execute_main_trap_outcome(module: &WasmBinaryModule) -> String {
    let bytes = module.to_bytes().expect("encode wasm module");
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("raise.wasm");
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
try {
  fn();
  process.stdout.write("returned");
} catch (error) {
  const trapped = error instanceof WebAssembly.RuntimeError && /unreachable/.test(error.message);
  process.stdout.write(trapped ? "trapped" : `other:${error}`);
}
"#,
    )
    .expect("write script");
    let output = Command::new("node").arg(&script_path).arg(&wasm_path).output().expect("spawn node to execute wasm");
    assert!(output.status.success(), "node 执行器自身失败: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// 正：`Raise` 编码为 `unreachable`，执行即 trap，不产生正常返回。
#[test]
fn wasm_raise_terminator_traps_at_runtime() {
    let module = lower_main(raising_function(false));
    let code = module.sections.iter().find(|section| section.id == 10).expect("code section");
    assert!(code.bytes.contains(&WasmOpcode::Unreachable.as_u8()), "raise 块必须编码 `unreachable`");
    assert_eq!(execute_main_trap_outcome(&module), "trapped", "未捕获 raise 在 wasm 上必须以 trap 终止");
}

/// 正：真实 `std::types::panic` 形态——resume 块带一个 `Resume = !` 形参——必须通过槽规划与调度发射。
#[test]
fn wasm_raise_with_resume_parameter_lowers() {
    let module = lower_main(raising_function(true));
    let code = module.sections.iter().find(|section| section.id == 10).expect("code section");
    assert!(code.bytes.contains(&WasmOpcode::Unreachable.as_u8()), "raise 块必须编码 `unreachable`");
    assert_eq!(execute_main_trap_outcome(&module), "trapped", "带 resume 形参的 raise 同样以 trap 终止");
}
