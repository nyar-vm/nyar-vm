//! wasm lane `f64` 端到端：整数↔浮点表示转换与库导出面。
//!
//! 这里不走字符串断言：每个用例都真实发射 wasm 库模块（`nyar.library_invoke`
//! section + JS glue 启动壳），再交给 Node 执行 `callExport`。只有 Node 里拿到
//! 真实的 `number` 结果（或真实的 wasm trap）才算通过。
//!
//! 正向覆盖：`i64 → f64`（有符号 `f64.convert_i64_s`）、`u64 → f64`（无符号
//! `f64.convert_i64_u`，用 2^63 区分符号性）、`f64 → i64`（`i64.trunc_f64_s`）、
//! 导出函数直接返回 `0.5` / `2.75`。
//!
//! 反向覆盖：NaN / 越界 `trunc` 必须触发 wasm trap，不得静默饱和。

use std::{collections::BTreeMap, process::Command};

use nyar::{HostProjectionBoundary, Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, Value, ValueRef},
    lowering::backends::wasm::lower_fragment_to_wasm_module_for,
    nyar_backend_wasi::{WasiPreview, WasmBinaryModule, WasmPackageKind},
};
use nyar_types::{InstructionId, NyarType, ProvenanceId, ValueOrigin};

const I64: NyarType = NyarType::Integer64 { signed: true };
const U64: NyarType = NyarType::Integer64 { signed: false };
const F64: NyarType = NyarType::Float64;

struct EmittedBridge {
    module: WasmBinaryModule,
    imports: Vec<(String, String)>,
    spec: String,
}

fn plan_for(function: ExecutableFunction) -> TargetExecutionPlan {
    let name = QualifiedName::new(vec![Identifier::new("echo")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(name, function)]));
    plan.testing_export_names_mut().insert(nyar_types::ItemInstanceId::from_index(0).expect("实例身份"), "echo".to_string());
    plan
}

fn emit(function: ExecutableFunction) -> EmittedBridge {
    let submission = FragmentSubmission::from_backend_plan(plan_for(function));
    let (module, imports) =
        lower_fragment_to_wasm_module_for(&submission, HostProjectionBoundary::WasmJsGlue, WasiPreview::Preview2, WasmPackageKind::Library)
            .expect("发射 f64 库模块");
    let spec = module
        .sections
        .iter()
        .find(|section| section.name.as_deref() == Some("nyar.library_invoke"))
        .map(|section| String::from_utf8_lossy(&section.bytes).to_string())
        .expect("库模块必须携带 nyar.library_invoke");
    EmittedBridge { module, imports, spec }
}

/// `echo(x: source) -> target { x as target }`。
fn cast_function(source: NyarType, target: NyarType) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "echo".to_string(),
        return_type: target.clone(),
        param_types: vec![source.clone()],
        value_types: BTreeMap::from([(ValueRef(0), source), (ValueRef(1), target.clone())]),
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: vec![Value { id: ValueRef(0), origin: ValueOrigin::Parameter { index: 0, name: "x".to_string() } }],
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: vec![ValueRef(0)],
            instructions: vec![Instruction {
                id: InstructionId::from_index(1).expect("指令身份"),
                results: vec![ValueRef(1)],
                kind: InstructionKind::Cast { source: Operand::Value(ValueRef(0)), ty: target },
                provenance: ProvenanceId::from_index(1).expect("provenance"),
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
        }],
        diagnostics: Vec::new(),
    }
}

/// `echo() -> f64 { 常量 }`：导出面直接返回浮点常量。
fn constant_f64_function(value: f64) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "echo".to_string(),
        return_type: F64,
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), F64)]),
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: vec![Value { id: ValueRef(0), origin: ValueOrigin::Temporary }],
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: vec![Instruction {
                id: InstructionId::from_index(1).expect("指令身份"),
                results: vec![ValueRef(0)],
                kind: InstructionKind::LoadConstant { constant: Constant::from(value), ty: Some(F64) },
                provenance: ProvenanceId::from_index(1).expect("provenance"),
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn spec_exports(spec: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(spec).expect("invoke spec 是 JSON")["exports"].clone()
}

/// 把模块 + 启动壳 + 用例脚本写进临时目录，交给 Node 执行，返回 stdout。
fn run_in_node(bridge: &EmittedBridge, driver_body: &str) -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("bridge.wasm"), bridge.module.to_bytes().expect("encode wasm")).expect("write wasm");
    let launcher = nyar_emitter::build_node_launcher("bridge", &bridge.imports, &[], true, Some(&bridge.spec));
    std::fs::write(dir.path().join("bridge.mjs"), launcher).expect("write launcher");
    let script = format!(
        r#"
import {{ callExport }} from "./bridge.mjs";
{diverge}
"#,
        diverge = driver_body
    );
    let script_path = dir.path().join("run.mjs");
    std::fs::write(&script_path, script).expect("write driver");
    let output = Command::new("node").arg(&script_path).current_dir(dir.path()).output().expect("spawn node");
    if !output.status.success() {
        panic!(
            "node bridge execute failed status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn run_json(bridge: &EmittedBridge, driver_body: &str) -> serde_json::Value {
    serde_json::from_str(&run_in_node(bridge, driver_body)).expect("driver JSON")
}

/// 导出函数直接返回 `f64`：schema 必须是原生 `F64`，Node 侧拿到 number。
#[test]
fn export_returning_f64_surfaces_native_number() {
    let bridge = emit(constant_f64_function(0.5));
    let returns = &spec_exports(&bridge.spec)["echo"]["returns"];
    assert_eq!(returns["kind"], "F64", "f64 返回不得退化成 anyref 或字符串: {returns}");

    let parsed = run_json(&bridge, r#"const out = {}; out.value = await callExport("echo"); process.stdout.write(JSON.stringify(out));"#);
    assert_eq!(parsed["value"], 0.5, "导出函数必须真实返回 0.5: {parsed}");
}

/// 第二个分数值，覆盖非二进制精确值的常规路径。
#[test]
fn export_returning_f64_keeps_fractional_value() {
    let bridge = emit(constant_f64_function(2.75));
    let parsed = run_json(&bridge, r#"const out = {}; out.value = await callExport("echo"); process.stdout.write(JSON.stringify(out));"#);
    assert_eq!(parsed["value"], 2.75, "导出函数必须真实返回 2.75: {parsed}");
}

/// G29 最小复现：`i64 → f64` 必须用有符号转换。
#[test]
fn i64_to_f64_uses_signed_conversion() {
    let bridge = emit(cast_function(I64, F64));
    let exports = spec_exports(&bridge.spec);
    assert_eq!(exports["echo"]["params"][0]["type"]["kind"], "I64", "参数载体必须是 I64: {exports}");
    assert_eq!(exports["echo"]["returns"]["kind"], "F64", "返回载体必须是 F64: {exports}");

    let parsed = run_json(
        &bridge,
        r#"const out = {};
out.positive = await callExport("echo", 3n);
out.negative = await callExport("echo", -7n);
process.stdout.write(JSON.stringify(out));"#,
    );
    assert_eq!(parsed["positive"], 3.0, "i64 → f64 正数必须精确: {parsed}");
    assert_eq!(parsed["negative"], -7.0, "i64 → f64 负数必须保留符号: {parsed}");
}

/// `u64 → f64` 必须用无符号转换：2^63 是最小判别点。
#[test]
fn u64_to_f64_uses_unsigned_conversion() {
    let bridge = emit(cast_function(U64, F64));
    let exports = spec_exports(&bridge.spec);
    assert_eq!(exports["echo"]["params"][0]["type"]["kind"], "I64", "u64 与 i64 共用 I64 物理载体: {exports}");
    assert_eq!(exports["echo"]["returns"]["kind"], "F64", "返回载体必须是 F64: {exports}");

    let parsed = run_json(
        &bridge,
        r#"const out = {};
out.boundary = await callExport("echo", (1n << 63n).toString());
out.max = await callExport("echo", ((1n << 64n) - 1n).toString());
process.stdout.write(JSON.stringify(out));"#,
    );
    // 2^63：有符号转换会得到 -9223372036854775808，无符号才是正数。
    assert_eq!(parsed["boundary"], 9223372036854775808.0, "u64 的 2^63 必须是正数（无符号转换）: {parsed}");
    assert_eq!(parsed["max"], 18446744073709551616.0, "u64::MAX 必须按无符号舍入到 2^64: {parsed}");
}

/// 反向 `f64 → i64`：wasm `trunc` 向零截断。
#[test]
fn f64_to_i64_truncates_toward_zero() {
    let bridge = emit(cast_function(F64, I64));
    let exports = spec_exports(&bridge.spec);
    assert_eq!(exports["echo"]["params"][0]["type"]["kind"], "F64", "参数载体必须是 F64: {exports}");
    assert_eq!(exports["echo"]["returns"]["kind"], "I64", "返回载体必须是 I64: {exports}");

    let parsed = run_json(
        &bridge,
        r#"const out = {};
out.positive = String(await callExport("echo", 2.75));
out.negative = String(await callExport("echo", -2.75));
process.stdout.write(JSON.stringify(out));"#,
    );
    assert_eq!(parsed["positive"], "2", "2.75 必须向零截断为 2: {parsed}");
    assert_eq!(parsed["negative"], "-2", "-2.75 必须向零截断为 -2: {parsed}");
}

/// 反向 `f64 → i64`：超出 i64 范围必须触发 wasm trap，不得静默饱和。
#[test]
fn f64_to_i64_out_of_range_traps() {
    let bridge = emit(cast_function(F64, I64));
    let parsed = run_json(
        &bridge,
        r#"const out = {};
try { out.value = String(await callExport("echo", 1e30)); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    assert!(parsed["error"].is_string(), "f64 → i64 越界必须触发结构化失败: {parsed}");
    assert!(parsed["value"].is_null(), "越界转换不得静默产出值: {parsed}");
}

/// 反向 `f64 → u64`：负数必须 trap（无符号截断不接受负值）。
#[test]
fn f64_to_u64_negative_traps() {
    let bridge = emit(cast_function(F64, U64));
    let parsed = run_json(
        &bridge,
        r#"const out = {};
try { out.value = String(await callExport("echo", -1.0)); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    assert!(parsed["error"].is_string(), "f64 → u64 的负数必须触发结构化失败: {parsed}");
    assert!(parsed["value"].is_null(), "越界转换不得静默产出值: {parsed}");
}
