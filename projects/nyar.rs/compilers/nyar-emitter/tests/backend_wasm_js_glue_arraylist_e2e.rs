//! `ArrayList<T>` 容器 ABI 桥的真实 wasm 端到端验证。
//!
//! 这里不走字符串断言：每个用例都真实发射 wasm 库模块（`nyar.library_invoke`
//! section + `struct.new`/`array.*` 导出 + JS glue 启动壳），再交给 Node 执行
//! `callExport`。只有 Node 里跑通「JS 数组 → VM 容器 → JS 数组」的往返才算通过。
//!
//! 正向覆盖：`ArrayList<i64>`、嵌套 `ArrayList<ArrayList<i64>>`、`ArrayList<utf8>`。
//! 反向覆盖：wire 形状错、元素类型错、i64 用 Number、i64 越界——全部必须结构化失败。

use std::collections::BTreeMap;
use std::process::Command;

use nyar::{HostProjectionBoundary, Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, ExecutableFunction, Operand, Terminator, Value, ValueRef},
    lowering::backends::wasm::lower_fragment_to_wasm_module_for,
    nyar_backend_wasi::{WasiPreview, WasmPackageKind, WasmBinaryModule},
};
use nyar_types::{
    AggregateLayout, FieldLayout, ItemInstanceId, NyarType, StorageKind, ValueOrigin,
};

const I64: NyarType = NyarType::Integer64 { signed: true };
const I32: NyarType = NyarType::Integer32 { signed: true };
const UTF8: NyarType = NyarType::Utf8;

fn arraylist_of(element: NyarType) -> NyarType {
    NyarType::Apply(Box::new(NyarType::Named(Identifier::new("std::collection::ArrayList"))), vec![element])
}

/// 两字段序列布局：一个 heap `Array<T>` + 一个整数计数（与 `ArrayList<T>` 形状一致）。
fn sequence_layout(id: u32, element: NyarType, length_ty: NyarType) -> AggregateLayout {
    AggregateLayout {
        id,
        name: "ArrayList".to_string(),
        namespace: "std::collection".to_string(),
        key: None,
        storage: StorageKind::Reference,
        size: 16,
        align: 8,
        fields: vec![
            FieldLayout { name: "_items".to_string(), ty: NyarType::Array(Box::new(element)), offset: 0, size: 8, align: 8 },
            FieldLayout { name: "_length".to_string(), ty: length_ty, offset: 8, size: 8, align: 8 },
        ],
    }
}

/// 恒等函数：`echo(x: T) -> T`。返回入参句柄，用来验证 glue 写进 VM 内存的
/// 容器/数组确实能被后续 demarshal 逐元素读回。
fn identity_function(ty: NyarType, layout: u32) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "echo".to_string(),
        return_type: ty.clone(),
        param_types: vec![ty.clone()],
        value_types: BTreeMap::from([(ValueRef(0), ty)]),
        value_layouts: BTreeMap::from([(ValueRef(0), layout)]),
        return_layout: Some(layout),
        entry: BlockRef(0),
        values: vec![Value { id: ValueRef(0), origin: ValueOrigin::Parameter { index: 0, name: "nums".to_string() } }],
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
            instructions: Vec::new(),
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

struct EmittedBridge {
    module: WasmBinaryModule,
    imports: Vec<(String, String)>,
    spec: String,
}

/// 发射 `echo` 的 wasm 库模块；`layouts` 的第一项是导出类型对应的布局。
fn emit_bridge(layouts: &[(NyarType, AggregateLayout)]) -> EmittedBridge {
    let (ty, layout) = layouts.first().expect("至少一个布局").clone();
    let name = QualifiedName::new(vec![Identifier::new("echo")]);
    let plan = TargetExecutionPlan::from_functions(BTreeMap::from([(name, identity_function(ty, layout.id))]));
    let mut plan = plan;
    for (ty, layout) in layouts {
        plan.testing_register_type_layout(ty.clone(), layout.clone());
    }
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("实例身份"), "echo".to_string());
    plan.testing_freeze_library_export_abis().expect("冻结库导出载体");
    let submission = FragmentSubmission::from_backend_plan(plan);
    let (module, imports) = lower_fragment_to_wasm_module_for(
        &submission,
        HostProjectionBoundary::WasmJsGlue,
        WasiPreview::Preview2,
        WasmPackageKind::Library,
    )
    .expect("发射 ArrayList 库模块");
    let spec = module
        .sections
        .iter()
        .find(|section| section.name.as_deref() == Some("nyar.library_invoke"))
        .map(|section| String::from_utf8_lossy(&section.bytes).to_string())
        .expect("库模块必须携带 nyar.library_invoke");
    EmittedBridge { module, imports, spec }
}

/// 把模块 + 启动壳 + 用例脚本写进临时目录，交给 Node 执行，返回 stdout（JSON）。
fn run_in_node(bridge: &EmittedBridge, driver_body: &str) -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("bridge.wasm");
    std::fs::write(&wasm_path, bridge.module.to_bytes().expect("encode wasm")).expect("write wasm");
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

fn spec_exports(spec: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(spec).expect("invoke spec 是 JSON")["exports"].clone()
}

#[test]
fn arraylist_i64_roundtrips_through_vm_memory() {
    let ty = arraylist_of(I64);
    let bridge = emit_bridge(&[(ty, sequence_layout(1, I64, I32))]);
    let exports = spec_exports(&bridge.spec);
    let echo = &exports["echo"]["params"][0]["type"];
    assert_eq!(echo["kind"], "Array", "ArrayList<i64> 必须投影为 Array: {echo}");
    assert_eq!(echo["element"]["kind"], "I64");
    assert_eq!(echo["container"]["items_field_index"], 0);
    assert_eq!(echo["container"]["length_field_index"], 1);
    assert_eq!(echo["container"]["length_type"]["kind"], "I32");

    let stdout = run_in_node(
        &bridge,
        r#"const big = (text) => BigInt(text);
const out = {};
try {
    const value = await callExport("echo", [big("1"), big("2"), big("3")]);
    out.value = value.map(String);
}
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "ArrayList<i64> 往返不得失败: {stdout}");
    assert_eq!(parsed["value"], serde_json::json!(["1", "2", "3"]));
}

#[test]
fn arraylist_arraylist_i64_roundtrips_nested() {
    let inner = arraylist_of(I64);
    let outer = arraylist_of(inner.clone());
    let bridge = emit_bridge(&[(outer.clone(), sequence_layout(2, inner.clone(), I32)), (inner, sequence_layout(1, I64, I32))]);
    let exports = spec_exports(&bridge.spec);
    let echo = &exports["echo"]["params"][0]["type"];
    assert_eq!(echo["kind"], "Array");
    assert_eq!(echo["element"]["kind"], "Array", "嵌套元素必须递归投影: {echo}");
    assert_eq!(echo["element"]["element"]["kind"], "I64");

    let stdout = run_in_node(
        &bridge,
        r#"const big = (text) => BigInt(text);
const out = {};
try {
    const value = await callExport("echo", [[big("1"), big("2")], [big("3")], []]);
    out.value = value.map((row) => row.map(String));
}
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "嵌套 ArrayList 往返不得失败: {stdout}");
    assert_eq!(parsed["value"], serde_json::json!([["1", "2"], ["3"], []]));
}

#[test]
fn arraylist_utf8_roundtrips() {
    let ty = arraylist_of(UTF8);
    let bridge = emit_bridge(&[(ty, sequence_layout(1, UTF8, I32))]);
    let exports = spec_exports(&bridge.spec);
    assert_eq!(exports["echo"]["params"][0]["type"]["element"]["kind"], "Utf8");

    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
try {
    const value = await callExport("echo", ["alpha", "beta", ""]);
    out.value = value;
}
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "ArrayList<utf8> 往返不得失败: {stdout}");
    assert_eq!(parsed["value"], serde_json::json!(["alpha", "beta", ""]));
}

#[test]
fn wire_shape_mismatch_fails_at_boundary_with_path()  {
    let ty = arraylist_of(I64);
    let bridge = emit_bridge(&[(ty, sequence_layout(1, I64, I32))]);
    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
try { await callExport("echo", 7); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    let message = parsed["error"].as_str().expect("必须结构化失败");
    assert!(message.contains("at arg0"), "失败信息必须带路径: {message}");
    assert!(message.contains("expected a JS Array"), "失败信息必须给出 wire 形状期望: {message}");
}

#[test]
fn element_type_mismatch_fails_at_boundary_with_element_path() {
    let ty = arraylist_of(I64);
    let bridge = emit_bridge(&[(ty, sequence_layout(1, I64, I32))]);
    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
try { await callExport("echo", [1n, "oops"]); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    let message = parsed["error"].as_str().expect("必须结构化失败");
    assert!(message.contains("arg0[1]"), "失败信息必须定位到元素: {message}");
    assert!(message.contains("I64"), "失败信息必须带元素类型: {message}");
}

#[test]
fn i64_as_number_fails_instead_of_being_coerced() {
    let ty = arraylist_of(I64);
    let bridge = emit_bridge(&[(ty, sequence_layout(1, I64, I32))]);
    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
try { await callExport("echo", [1n, 2]); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    let message = parsed["error"].as_str().expect("Number 传入 i64 必须结构化失败");
    assert!(message.contains("arg0[1]"), "失败信息必须定位到元素: {message}");
    assert!(message.contains("requires a BigInt"), "失败信息必须说明 BigInt 合同: {message}");
}

#[test]
fn i64_out_of_range_fails_at_boundary() {
    let ty = arraylist_of(I64);
    let bridge = emit_bridge(&[(ty, sequence_layout(1, I64, I32))]);
    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
try { await callExport("echo", ["18446744073709551616"]); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    let message = parsed["error"].as_str().expect("越界 i64 必须结构化失败");
    assert!(message.contains("outside the 64-bit range"), "失败信息必须说明越界: {message}");
}
