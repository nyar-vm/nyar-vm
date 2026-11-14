//! 语言数组 ABI 桥与名义引用边界的真实 wasm 端到端验证。
//!
//! 每个用例发射真实 wasm 库模块与 JS glue，再经 Node 调用导出。
//! 数组覆盖标量、嵌套、Option 与严格的元素校验；名义引用实例不因
//! 数组与整数字段的布局形状获得宿主数组投影或字段重建授权。

use std::{collections::BTreeMap, process::Command};

use acorn_wasm::{VALTYPE_ANYREF, WasmOpcode, encode_ref_null_anyref};
use nyar::{HostProjectionBoundary, Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, ExecutableFunction, Operand, Terminator, Value, ValueRef},
    lowering::backends::wasm::lower_fragment_to_wasm_module_for,
    nyar_backend_wasi::{WasiPreview, WasmBinaryModule, WasmPackageKind, WasmSection},
};
use nyar_types::{AggregateLayout, FieldLayout, ItemInstanceId, NyarType, StorageKind, ValueOrigin};

const I64: NyarType = NyarType::Integer64 { signed: true };
const I32: NyarType = NyarType::Integer32 { signed: true };
const UTF8: NyarType = NyarType::Utf8;

fn array_of(element: NyarType) -> NyarType {
    NyarType::Array(Box::new(element))
}

/// 数组字段和整数字段只是物理布局，不声明任何序列协议。
fn reference_layout(id: u32, name: &str) -> AggregateLayout {
    AggregateLayout {
        id,
        name: name.to_string(),
        namespace: "demo".to_string(),
        key: None,
        storage: StorageKind::Reference,
        fields: vec![
            FieldLayout { name: "samples".to_string(), ty: array_of(I64) },
            FieldLayout { name: "version".to_string(), ty: I64 },
        ],
    }
}

/// 恒等函数：`echo(x: T) -> T`。返回入参句柄，用来验证 glue 写进 VM 内存的
/// 容器/数组确实能被后续 demarshal 逐元素读回。
fn identity_function(ty: NyarType, layout: Option<u32>) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "echo".to_string(),
        return_type: ty.clone(),
        param_types: vec![ty.clone()],
        value_types: BTreeMap::from([(ValueRef(0), ty)]),
        value_layouts: layout.map(|layout| BTreeMap::from([(ValueRef(0), layout)])).unwrap_or_default(),
        return_layout: layout,
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
    let plan = TargetExecutionPlan::from_functions(BTreeMap::from([(name, identity_function(ty, Some(layout.id)))]));
    let mut plan = plan;
    for (ty, layout) in layouts {
        plan.testing_register_type_layout(ty.clone(), layout.clone());
    }
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(0).expect("实例身份"), "echo".to_string());
    let submission = FragmentSubmission::from_backend_plan(plan);
    let (module, imports) =
        lower_fragment_to_wasm_module_for(&submission, HostProjectionBoundary::WasmJsGlue, WasiPreview::Preview2, WasmPackageKind::Library)
            .expect("发射库模块");
    let spec = module
        .sections
        .iter()
        .find(|section| section.name.as_deref() == Some("nyar.library_invoke"))
        .map(|section| String::from_utf8_lossy(&section.bytes).to_string())
        .expect("库模块必须携带 nyar.library_invoke");
    EmittedBridge { module, imports, spec }
}

/// 发射无布局的直接类型（如 `Nullable(I64)`）：槽是 anyref，但 schema 是 `Option`。
fn emit_plain(ty: NyarType) -> EmittedBridge {
    let name = QualifiedName::new(vec![Identifier::new("echo")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(name, identity_function(ty, None))]));
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(0).expect("实例身份"), "echo".to_string());
    let submission = FragmentSubmission::from_backend_plan(plan);
    let (module, imports) =
        lower_fragment_to_wasm_module_for(&submission, HostProjectionBoundary::WasmJsGlue, WasiPreview::Preview2, WasmPackageKind::Library)
            .expect("发射单值库模块");
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
fn array_and_integer_fields_do_not_authorize_reference_reconstruction() {
    for (index, name) in ["ArrayList", "Measurement"].iter().enumerate() {
        let ty = NyarType::Nominal { layout: (index + 1) as u32, spelling: Identifier::new(&format!("demo::{name}")) };
        let bridge = emit_bridge(&[(ty, reference_layout((index + 1) as u32, name))]);
        let exports = spec_exports(&bridge.spec);
        let schema = &exports["echo"]["params"][0]["type"];
        assert_eq!(schema["kind"], "InstanceHandle", "字段形状不能声明序列语义: {schema}");
        assert_eq!(exports["echo"]["returns"]["kind"], "InstanceHandle");
        for value in ["[1n, 2n]", "{ samples: [1n], version: 99n }"] {
            let message = call_error(&bridge, value);
            assert!(message.contains("at arg0"), "拒绝必须包含入参路径: {message}");
            assert!(message.contains("expected the opaque handle"), "宿主不得按数组或字段重建引用实例: {message}");
        }
    }
}

#[test]
fn array_i64_roundtrips_through_vm_memory() {
    let ty = array_of(I64);
    let bridge = emit_plain(ty);
    let exports = spec_exports(&bridge.spec);
    let echo = &exports["echo"]["params"][0]["type"];
    assert_eq!(echo["kind"], "Array", "Array<i64> 必须投影为 Array: {echo}");
    assert_eq!(echo["element"]["kind"], "I64");
    assert!(echo.get("container").is_none(), "数组 ABI 不携带名义对象重建合同: {echo}");

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
    assert!(parsed["error"].is_null(), "Array<i64> 往返不得失败: {stdout}");
    assert_eq!(parsed["value"], serde_json::json!(["1", "2", "3"]));
}

#[test]
fn nested_arrays_i64_roundtrip() {
    let inner = array_of(I64);
    let outer = array_of(inner.clone());
    let bridge = emit_plain(outer);
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
    assert!(parsed["error"].is_null(), "嵌套 Array 往返不得失败: {stdout}");
    assert_eq!(parsed["value"], serde_json::json!([["1", "2"], ["3"], []]));
}

#[test]
fn array_utf8_roundtrips() {
    let ty = array_of(UTF8);
    let bridge = emit_plain(ty);
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
    assert!(parsed["error"].is_null(), "Array<utf8> 往返不得失败: {stdout}");
    assert_eq!(parsed["value"], serde_json::json!(["alpha", "beta", ""]));
}

/// `Utf8Text` 形状：一个 heap `Array<Integer8>` 字节字段（与 `std` 的 managed 字符串一致）。
fn utf8_text_layout(id: u32) -> AggregateLayout {
    AggregateLayout {
        id,
        name: "Utf8Text".to_string(),
        namespace: "std".to_string(),
        key: None,
        storage: StorageKind::Reference,
        fields: vec![FieldLayout {
            name: "_repr".to_string(),
            ty: NyarType::Array(Box::new(NyarType::Integer8 { signed: false })),
        }],
    }
}

/// `returns: Utf8` 顶层返回值：VM 内部返回 managed 字符串 struct（anyref），导出边界
/// 必须按 schema 物化为宿主 i32 句柄，再经 glue 解包回 JS 字符串。修复前导出物理签名是
/// `(result anyref)`，glue 按 i32 解包拿到 null-proto struct 并抛 TypeError。
#[test]
fn utf8_return_roundtrips_as_host_string() {
    let bridge = emit_bridge(&[(UTF8, utf8_text_layout(3))]);
    let exports = spec_exports(&bridge.spec);
    assert_eq!(exports["echo"]["returns"]["kind"], "Utf8");
    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
try { out.value = await callExport("echo", "hello 世界"); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "Utf8 返回值物化不得失败: {stdout}");
    assert_eq!(parsed["value"], serde_json::json!("hello 世界"));
}

#[test]
fn utf8_return_without_layout_stays_i32_and_fails_on_missing_handle() {
    // 未登记 managed 字符串 layout 时返回槽就是宿主 i32 句柄；恒等函数把入参句柄原样
    // 返回，因此必须往返成功而不是被误当 anyref 解包。
    let bridge = emit_plain(UTF8);
    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
try { out.value = await callExport("echo", "hello 世界"); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "i32 句柄返回值不得失败: {stdout}");
    assert_eq!(parsed["value"], serde_json::json!("hello 世界"));
}

#[test]
fn wire_shape_mismatch_fails_at_boundary_with_path() {
    let ty = array_of(I64);
    let bridge = emit_plain(ty);
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
    let ty = array_of(I64);
    let bridge = emit_plain(ty);
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
    let ty = array_of(I64);
    let bridge = emit_plain(ty);
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
    let ty = array_of(I64);
    let bridge = emit_plain(ty);
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

// ── 标量元素：缺值 / 类型不符必须带路径与期望类型失败闭合 ─────────────────────

fn array_case(element: NyarType) -> EmittedBridge {
    emit_plain(array_of(element))
}

/// 调用 `echo`，把抛出的错误消息交回 Rust（空串表示没有报错）。
fn call_error(bridge: &EmittedBridge, arg_js: &str) -> String {
    let driver = format!(
        r#"const out = {{}};
try {{ await callExport("echo", {arg_js}); }}
catch (error) {{ out.error = String(error && error.message ? error.message : error); }}
process.stdout.write(JSON.stringify(out));"#
    );
    let stdout = run_in_node(bridge, &driver);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    parsed["error"].as_str().unwrap_or("").to_string()
}

#[test]
fn i32_element_null_fails_with_path_and_expected_type() {
    let bridge = array_case(I32);
    let message = call_error(&bridge, "[1, null, 3]");
    assert!(message.contains("arg0[1]"), "失败信息必须定位到元素: {message}");
    assert!(message.contains("I32"), "失败信息必须带期望类型: {message}");
    assert!(message.contains("expected an integer number"), "失败信息必须说明期望载体: {message}");
}

#[test]
fn f64_element_string_fails_with_path_and_expected_type() {
    let bridge = array_case(NyarType::Float64);
    let message = call_error(&bridge, "[1.5, \"2.5\"]");
    assert!(message.contains("arg0[1]"), "失败信息必须定位到元素: {message}");
    assert!(message.contains("F64"), "失败信息必须带期望类型: {message}");
    assert!(message.contains("expected a finite number"), "失败信息必须说明期望载体: {message}");
}

#[test]
fn char_element_multi_codepoint_fails_with_path_and_expected_type() {
    let bridge = array_case(NyarType::Character);
    let message = call_error(&bridge, "[\"ab\"]");
    assert!(message.contains("arg0[0]"), "失败信息必须定位到元素: {message}");
    assert!(message.contains("Char"), "失败信息必须带期望类型: {message}");
    assert!(message.contains("expected a single Unicode code point"), "失败信息必须说明 Char 合同: {message}");
}

#[test]
fn undefined_element_fails_with_path_and_expected_type() {
    let bridge = array_case(I32);
    let message = call_error(&bridge, "[undefined]");
    assert!(message.contains("arg0[0]"), "失败信息必须定位到元素: {message}");
    assert!(message.contains("undefined"), "失败信息必须区分缺值: {message}");
    assert!(message.contains("I32"), "失败信息必须带期望类型: {message}");
}

// ── Option 标量内层：真实降级是 niche（None = ref.null，Some(标量) = [i32]/[i64] box）──
// wire 规则：None → null，Some(v) → v。以下全部是真发射 wasm + Node 实跑。

fn nullable_of(inner: NyarType) -> NyarType {
    NyarType::Nullable(Box::new(inner))
}

fn call_value(bridge: &EmittedBridge, arg_js: &str) -> serde_json::Value {
    let driver = format!(
        r#"const out = {{}};
try {{ out.value = await callExport("echo", {arg_js}); }}
catch (error) {{ out.error = String(error && error.message ? error.message : error); }}
process.stdout.write(JSON.stringify(out, (_, v) => typeof v === "bigint" ? String(v) : v));"#
    );
    serde_json::from_str(&run_in_node(bridge, &driver)).expect("driver JSON")
}

#[test]
fn option_i64_roundtrips_as_parameter_and_return() {
    let bridge = emit_plain(nullable_of(I64));
    let exports = spec_exports(&bridge.spec);
    let schema = &exports["echo"]["params"][0]["type"];
    assert_eq!(schema["kind"], "Option");
    assert_eq!(schema["inner"]["kind"], "I64");
    assert_eq!(schema["box"]["payload"], "i64", "Option<i64> 必须携带 [i64] 装箱合同: {schema}");

    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
const none = await callExport("echo", null);
out.none = none === null ? "null" : none;
const some = await callExport("echo", 5n);
out.some = typeof some === "bigint" ? String(some) : some;
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "Option<i64> 往返不得失败: {stdout}");
    assert_eq!(parsed["none"], "null", "None 必须以 null 往返: {stdout}");
    assert_eq!(parsed["some"], "5", "Some(i64) 必须以 BigInt 往返: {stdout}");
}

#[test]
fn option_i64_container_elements_roundtrip_mixed_none_and_some() {
    let bridge = array_case(nullable_of(I64));
    let exports = spec_exports(&bridge.spec);
    let element = &exports["echo"]["params"][0]["type"]["element"];
    assert_eq!(element["kind"], "Option");
    assert_eq!(element["inner"]["kind"], "I64");
    assert_eq!(element["box"]["payload"], "i64", "容器元素同样必须携带装箱合同: {element}");

    let parsed = call_value(&bridge, "[null, 5n, null, 7n]");
    assert!(parsed["error"].is_null(), "Array<Option<i64>> 往返不得失败: {parsed}");
    assert_eq!(parsed["value"], serde_json::json!([null, "5", null, "7"]));
}

#[test]
fn option_utf8_roundtrips_as_parameter_and_return() {
    let bridge = emit_plain(nullable_of(UTF8));
    let exports = spec_exports(&bridge.spec);
    let schema = &exports["echo"]["params"][0]["type"];
    assert_eq!(schema["kind"], "Option");
    assert_eq!(schema["inner"]["kind"], "Utf8");
    assert_eq!(schema["box"]["payload"], "i32", "Utf8 在 JS glue 是 i32 宿主句柄: {schema}");

    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
const none = await callExport("echo", null);
out.none = none === null ? "null" : none;
const some = await callExport("echo", "中文😀");
out.some = some;
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "Option<utf8> 往返不得失败: {stdout}");
    assert_eq!(parsed["none"], "null");
    assert_eq!(parsed["some"], "中文😀");
}

#[test]
fn option_utf8_container_elements_roundtrip_mixed() {
    let bridge = array_case(nullable_of(UTF8));
    let parsed = call_value(&bridge, "[\"中文😀\", null, \"\"]");
    assert!(parsed["error"].is_null(), "Array<Option<utf8>> 往返不得失败: {parsed}");
    assert_eq!(parsed["value"], serde_json::json!(["中文😀", null, ""]));
}

#[test]
fn option_undefined_param_is_rejected_with_path() {
    let bridge = emit_plain(nullable_of(I64));
    let message = call_error(&bridge, "undefined");
    assert!(message.contains("at arg0"), "失败信息必须带入参路径: {message}");
    assert!(message.contains("undefined is not the None wire form"), "失败信息必须说明 wire 规则: {message}");
    assert!(message.contains("Option"), "失败信息必须带 Option schema: {message}");
}

#[test]
fn option_inner_type_mismatch_fails_at_arg_path() {
    let bridge = emit_plain(nullable_of(I64));
    let message = call_error(&bridge, "5");
    assert!(message.contains("at arg0"), "失败信息必须带入参路径: {message}");
    assert!(message.contains("I64"), "失败信息必须带内层期望类型: {message}");
    assert!(message.contains("requires a BigInt"), "失败信息必须说明内层载体合同: {message}");
}

#[test]
fn option_container_element_mismatch_fails_at_element_path() {
    let bridge = array_case(nullable_of(I64));
    let message = call_error(&bridge, "[null, 5n, \"oops\"]");
    assert!(message.contains("arg0[2]"), "失败信息必须定位到元素: {message}");
    assert!(message.contains("I64"), "失败信息必须带内层期望类型: {message}");
}

#[test]
fn option_array_where_scalar_payload_expected_fails_at_arg_path() {
    let bridge = emit_plain(nullable_of(I64));
    let message = call_error(&bridge, "[]");
    assert!(message.contains("at arg0"), "失败信息必须带入参路径: {message}");
    assert!(message.contains("I64"), "失败信息必须带内层期望类型: {message}");
}

/// 手工 wasm：`() -> anyref` 直接 `ref.null`，用来实测返回值侧的 Option 反编组。
fn nullref_module(export: &str) -> WasmBinaryModule {
    let mut module = WasmBinaryModule::new();
    module.sections.push(WasmSection { id: 1, name: None, bytes: vec![1, 0x60, 0, 1, VALTYPE_ANYREF] });
    module.sections.push(WasmSection { id: 3, name: None, bytes: vec![1, 0] });
    // Export 段（id=7）：1 个函数导出（count / name_len / name / kind=func / func_idx）。
    let mut exports_payload = vec![1u8];
    exports_payload.push(u8::try_from(export.len()).expect("export name"));
    exports_payload.extend(export.as_bytes());
    exports_payload.extend([0x00, 0x00]);
    module.sections.push(WasmSection { id: 7, name: None, bytes: exports_payload });
    let mut body = vec![0u8];
    encode_ref_null_anyref(&mut body);
    WasmOpcode::End.encode(&mut body);
    let mut code = vec![1u8, u8::try_from(body.len()).expect("body len")];
    code.append(&mut body);
    module.sections.push(WasmSection { id: 10, name: None, bytes: code });
    module
}

#[test]
fn option_return_none_demarshals_to_null() {
    let spec = r#"{"version":0,"exports":{"noop":{"params":[],"returns":{"kind":"Option","inner":{"kind":"I64"},"box":{"new":"nyar$option$new$i64$0","get":"nyar$option$get$i64$0","payload":"i64"}}}}}"#;
    let bridge = EmittedBridge { module: nullref_module("noop"), imports: Vec::new(), spec: spec.to_string() };
    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
try { out.value = await callExport("noop", []); }
catch (error) { out.error = String(error && error.message ? error.message : error); }
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "空引用必须反编组为 null: {stdout}");
    assert!(parsed["value"].is_null());
}

#[test]
fn option_f64_roundtrips_as_parameter_and_return() {
    let bridge = emit_plain(nullable_of(NyarType::Float64));
    let exports = spec_exports(&bridge.spec);
    let schema = &exports["echo"]["params"][0]["type"];
    assert_eq!(schema["kind"], "Option");
    assert_eq!(schema["inner"]["kind"], "F64");
    assert_eq!(schema["box"]["payload"], "f64", "Option<F64> 必须携带 [f64] 装箱合同: {schema}");

    let stdout = run_in_node(
        &bridge,
        r#"const out = {};
const none = await callExport("echo", null);
out.none = none === null ? "null" : none;
const some = await callExport("echo", 2.5);
out.some = some;
const negative = await callExport("echo", -1.25);
out.negative = negative;
process.stdout.write(JSON.stringify(out));"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("driver JSON");
    assert!(parsed["error"].is_null(), "Option<F64> 往返不得失败: {stdout}");
    assert_eq!(parsed["none"], "null", "None 必须以 null 往返: {stdout}");
    assert_eq!(parsed["some"], 2.5, "Some(f64) 必须以 Number 往返: {stdout}");
    assert_eq!(parsed["negative"], -1.25, "Some(f64) 负值同样必须保真: {stdout}");
}

#[test]
fn option_f64_container_elements_roundtrip_mixed_none_and_some() {
    let bridge = array_case(nullable_of(NyarType::Float64));
    let exports = spec_exports(&bridge.spec);
    let element = &exports["echo"]["params"][0]["type"]["element"];
    assert_eq!(element["kind"], "Option");
    assert_eq!(element["inner"]["kind"], "F64");
    assert_eq!(element["box"]["payload"], "f64", "容器元素同样必须携带 [f64] 装箱合同: {element}");

    let parsed = call_value(&bridge, "[null, 2.5, null, -0.75]");
    assert!(parsed["error"].is_null(), "Array<Option<F64>> 往返不得失败: {parsed}");
    assert_eq!(parsed["value"], serde_json::json!([null, 2.5, null, -0.75]));
}

#[test]
fn option_f64_inner_type_mismatch_fails_at_element_path() {
    let bridge = array_case(nullable_of(NyarType::Float64));
    let message = call_error(&bridge, "[null, 2.5, \"oops\"]");
    assert!(message.contains("arg0[2]"), "失败信息必须定位到元素: {message}");
    assert!(message.contains("F64"), "失败信息必须带内层期望类型: {message}");
    assert!(message.contains("expected a finite number"), "失败信息必须说明 F64 载体合同: {message}");
}

#[test]
fn option_scalar_without_box_contract_fails_closed_on_marshal() {
    // 后端只为 i32/i64/f64 注册了 box struct；f32 载荷没有物理表示，schema 也不得编合同。
    let bridge = emit_plain(nullable_of(NyarType::Float32));
    let exports = spec_exports(&bridge.spec);
    let schema = &exports["echo"]["params"][0]["type"];
    assert_eq!(schema["kind"], "Option");
    assert_eq!(schema["inner"]["kind"], "F32");
    assert!(schema["box"].is_null(), "无 box struct 的载荷不得携带合同: {schema}");

    let message = call_error(&bridge, "1.5");
    assert!(message.contains("at arg0"), "失败信息必须带入参路径: {message}");
    assert!(message.contains("Option<F32>"), "失败信息必须带 schema 内层类型: {message}");
    assert!(message.contains("no box/unbox contract"), "失败信息必须说明合同缺口: {message}");
}
