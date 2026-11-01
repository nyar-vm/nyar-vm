//! WASM 宿主字符串边界回归：managed 字符串 → 宿主 i32 句柄的转换必须在**目标计划阶段冻结**
//! （合同 §5），编码阶段只消费 `ManagedUtf8ToHostHandlePlan` 配方表。
//!
//! 崩溃现场：`env::utf8_length` 声明 i32 宿主句柄参数，但实参 SSA 槽因 `Utf8Text`
//! （`class Utf8Text { _repr: [u8] }`）layout 已登记进 gc struct 注册表而是 anyref。
//! 修复前 `emit_operand_coerced` 在 carrier 失配（expected=i32 actual=anyref）处 panic。
//!
//! 锁定三件事：
//! - 正向：managed 字符串命中冻结配方 → lowering 成功，合成导入 `env::host_intern_utf8`
//!   按需出现在 import 段；
//! - 按需：未绑定注册 layout 的宿主字符串句柄仍走 i32 ABI，**不得**合成新宿主导入；
//! - 负向：已注册 gc struct 但 layout 缺少整数字节数组字段 → 冻结期结构化失败，
//!   禁止 panic / 运行期 coerce 掩盖。
//!
//! 反方向（库导出边界）同样锁定三件事：形参绑定 `Utf8Text` layout 的库导出必须合成
//! 物化 wrapper（物理签名 anyref 槽 → i32 句柄，导入 `env::host_utf8_extract` 按需追加，
//! 导出重定向到 wrapper）；未绑定 layout 的句柄形参直通、不合成 wrapper；缺少冻结 ABI
//! 载体合同必须结构化失败（崩溃现场 `RuntimeError: illegal cast` 的编译期锁定）。

use acorn_wasm::{
    WasmBinaryModule, WasmExternalKind, WasmTypeKind, WasmValueType, parse_code_section, parse_export_section, parse_import_section,
    parse_type_section,
};
use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldLayout, ImportIndex, ItemInstanceId, LayoutId, NyarType, StorageKind};
use std::collections::BTreeMap;

const UTF8: NyarType = NyarType::Utf8;
const I32: NyarType = NyarType::Integer32 { signed: true };

const UTF8_TEXT_LAYOUT_ID: LayoutId = 7;
const NO_BYTES_LAYOUT_ID: LayoutId = 9;

fn byte_array() -> NyarType {
    NyarType::Array(Box::new(NyarType::Integer8 { signed: false }))
}

fn long_array() -> NyarType {
    NyarType::Array(Box::new(NyarType::Integer64 { signed: true }))
}

/// 单字段聚合 layout：`_repr: <bytes_field>`，storage Reference → 必进 gc struct 注册表。
fn layout_plan(id: LayoutId, name: &str, bytes_field: NyarType) -> AggregateLayoutPlan {
    AggregateLayoutPlan {
        layouts: vec![AggregateLayout {
            id,
            name: name.to_string(),
            namespace: String::new(),
            key: None,
            storage: StorageKind::Reference,
            size: 4,
            align: 4,
            fields: vec![FieldLayout { name: "_repr".to_string(), ty: bytes_field, offset: 0, size: 4, align: 4 }],
        }],
        ..Default::default()
    }
}

fn utf8_length_callee() -> ItemInstanceId {
    ItemInstanceId::from_index(203).expect("测试 callee 身份")
}

/// provider micro 消费的宿主导入：`env::utf8_length(text: Utf8) -> i32`。
/// import ABI 按 `wasm_param_value_type` 编译为 `[i32] -> [i32]` 宿主句柄签名。
fn utf8_length_import() -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(0).expect("导入下标"),
        link: nyar::ExternalImportLink::host(Some(Identifier::new("wasm")), vec!["env".into(), "utf8_length".into()]),
        parameter_types: vec![UTF8],
        return_type: I32,
        target_operator: None,
    }
}

/// `main(text: Utf8)`：%1 = `utf8_length(%0)`；return %1。
/// `bound_layout` 决定实参槽是否携带注册 gc struct layout——即崩溃现场的触发开关。
fn utf8_length_call_main(bound_layout: Option<LayoutId>) -> ExecutableFunction {
    let call = Instruction::from_kind(InstructionKind::Call {
        callee: Operand::Item(utf8_length_callee()),
        arguments: vec![Operand::Value(ValueRef(0))],
    });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0)],
        instructions: vec![Instruction { results: vec![ValueRef(1)], ..call }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
    };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: I32,
        param_types: vec![UTF8],
        value_types: BTreeMap::from([(ValueRef(0), UTF8), (ValueRef(1), I32)]),
        value_layouts: bound_layout.map(|id| BTreeMap::from([(ValueRef(0), id)])).unwrap_or_default(),
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

fn lower(function: ExecutableFunction, layouts: AggregateLayoutPlan) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("host_string_boundary").testing_entry_operation(main_name);
    plan.testing_aggregate_layouts(layouts);
    plan.testing_register_import(utf8_length_callee(), utf8_length_import());
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main")
}

fn import_keys(module: &WasmBinaryModule) -> Vec<(String, String)> {
    parse_import_section(module).into_iter().map(|item| (item.module, item.field)).collect()
}

/// 正向：`Utf8Text`（`_repr: [u8]`）layout 绑定进实参槽时，宿主边界必须消费冻结配方
/// 转换成功，且合成导入 `env::host_intern_utf8` 按需出现在 import 段。
/// 修复前此处 carrier 失配直接 panic（`WASM ABI carrier mismatch`）。
#[test]
fn wasm_managed_utf8_host_boundary_freezes_recipe_and_synthesizes_intern_import() {
    let module = lower(utf8_length_call_main(Some(UTF8_TEXT_LAYOUT_ID)), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()))
        .expect("managed 字符串命中冻结配方时宿主边界转换必须成功");
    let imports = import_keys(&module);
    assert!(imports.contains(&("env".to_string(), "utf8_length".to_string())), "原宿主导入必须保留: {imports:?}");
    assert!(
        imports.contains(&("env".to_string(), "host_intern_utf8".to_string())),
        "冻结配方必须按需合成 `env::host_intern_utf8`: {imports:?}"
    );
}

/// 按需合成：未绑定注册 layout 的宿主字符串句柄仍走 i32 ABI（`const_utf8` 语义），
/// 不得凭空追加 `env::host_intern_utf8`——转换导入只服务确有 managed 字符串的模块。
#[test]
fn wasm_host_string_handle_without_layout_does_not_synthesize_intern_import() {
    let module = lower(utf8_length_call_main(None), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()))
        .expect("未绑定 layout 的宿主字符串句柄必须继续走 i32 ABI");
    let imports = import_keys(&module);
    assert!(!imports.contains(&("env".to_string(), "host_intern_utf8".to_string())), "无 managed 字符串时不得合成转换导入: {imports:?}");
}

/// 负向：layout 已注册 gc struct 但没有整数字节数组字段时，转换配方无法在冻结期生成，
/// 必须结构化失败并指明缺失事实；禁止 panic 或把 anyref coerce 成 i32 留给运行期 trap。
#[test]
fn wasm_managed_layout_without_byte_array_field_fails_structured_at_freeze() {
    let error = lower(utf8_length_call_main(Some(NO_BYTES_LAYOUT_ID)), layout_plan(NO_BYTES_LAYOUT_ID, "HostHandleText", long_array()))
        .expect_err("缺少整数字节数组字段的 layout 必须在目标计划冻结期结构化失败");
    assert!(error.contains("缺少整数字节数组字段"), "错误必须指明冻结失败的缺失事实: {error}");
}

/// 库导出场景：`main(text: Utf8)` 登记公开导出名 `romanToInt`（崩溃现场形状——形参 SSA
/// 值绑定 `Utf8Text` layout 使物理槽为 anyref，而宿主按冻结库 ABI 传 i32 句柄）。
/// `freeze_abis` 控制是否补齐库导出 ABI 载体冻结，模拟合同缺失路径。
fn lower_library(function: ExecutableFunction, layouts: AggregateLayoutPlan, freeze_abis: bool) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("host_string_boundary").testing_entry_operation(main_name);
    plan.testing_aggregate_layouts(layouts);
    plan.testing_register_import(utf8_length_callee(), utf8_length_import());
    let instance = *plan.instances().first().expect("测试库导出实例");
    plan.testing_wasm_export_names_mut().insert(instance, "romanToInt".to_string());
    if freeze_abis {
        plan.testing_freeze_library_export_abis()?;
    }
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main")
}

fn func_import_count(module: &WasmBinaryModule) -> u32 {
    parse_import_section(module).into_iter().filter(|item| item.kind == WasmExternalKind::Func).count() as u32
}

fn decode_uleb(bytes: &[u8], pos: &mut usize) -> u32 {
    let mut value = 0u32;
    let mut shift = 0;
    loop {
        let byte = bytes[*pos];
        *pos += 1;
        value |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            break value;
        }
        shift += 7;
    }
}

/// function 段（id 3）是 typeidx 序列：按函数索引序解码每个函数的物理签名类型下标。
fn function_section_type_indices(module: &WasmBinaryModule) -> Vec<u32> {
    let section = module.sections.iter().find(|item| item.id == 3).expect("function 段必须存在");
    let mut pos = 0;
    let count = decode_uleb(&section.bytes, &mut pos);
    (0..count).map(|_| decode_uleb(&section.bytes, &mut pos)).collect()
}

fn func_type(module: &WasmBinaryModule, type_index: u32) -> (Vec<WasmValueType>, Vec<WasmValueType>) {
    let entry = parse_type_section(module).into_iter().find(|item| item.index == type_index as usize).expect("函数必须有 functype 条目");
    match entry.kind {
        WasmTypeKind::Func { params, results } => (params, results),
        other => panic!("type {type_index} 应为 functype，实际 {other:?}"),
    }
}

/// 正向：库导出形参绑定 `Utf8Text` layout 时，导出边界必须合成物化 wrapper——
/// 物理签名把 anyref 槽替换为 i32 句柄、`env::host_utf8_extract` 按需追加、导出
/// 重定向到 wrapper（内部函数保留 anyref 签名）。修复前宿主句柄直送 anyref 槽，
/// 函数体内首个 `ref.cast` 命中包装句柄 → 运行期 `illegal cast`。
#[test]
fn wasm_library_export_host_handle_materializes_managed_utf8_through_synthesized_wrapper() {
    let module =
        lower_library(utf8_length_call_main(Some(UTF8_TEXT_LAYOUT_ID)), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()), true)
            .expect("库导出宿主句柄形参必须按冻结配方物化为 managed 字符串");
    let imports = import_keys(&module);
    assert!(
        imports.contains(&("env".to_string(), "host_utf8_extract".to_string())),
        "冻结配方必须按需合成 `env::host_utf8_extract`: {imports:?}"
    );

    let export = parse_export_section(&module).into_iter().find(|item| item.name == "romanToInt").expect("公开导出名必须存在");
    assert_eq!(export.kind, WasmExternalKind::Func);
    assert_eq!(export.index, func_import_count(&module) + 1, "导出必须重定向到内部函数之后的 wrapper");

    assert_eq!(parse_code_section(&module).len(), 3, "函数体必须是 main + wrapper + cabi_realloc");
    let type_indices = function_section_type_indices(&module);
    assert_eq!(type_indices.len(), 3);
    let (main_params, _) = func_type(&module, type_indices[0]);
    assert_eq!(main_params, vec![WasmValueType::AnyRef], "内部函数物理槽必须保持 managed anyref");
    let (wrapper_params, wrapper_results) = func_type(&module, type_indices[1]);
    assert_eq!(wrapper_params, vec![WasmValueType::I32], "wrapper 物理形参槽必须是 i32 宿主句柄");
    assert_eq!(wrapper_results, vec![WasmValueType::I32], "wrapper 必须原样透传内部函数返回");
}

/// 按需：未绑定注册 layout 的库导出形参本就是宿主 i32 句柄，直通即可——
/// 不得合成 `env::host_utf8_extract` 导入，也不得多生成 wrapper 函数体。
#[test]
fn wasm_library_export_handle_parameter_without_layout_does_not_synthesize_wrapper() {
    let module = lower_library(utf8_length_call_main(None), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()), true)
        .expect("未绑定 layout 的库导出句柄形参必须直通");
    let imports = import_keys(&module);
    assert!(!imports.contains(&("env".to_string(), "host_utf8_extract".to_string())), "无物化需求时不得合成 extract 导入: {imports:?}");
    assert_eq!(parse_code_section(&module).len(), 2, "函数体必须只有 main + cabi_realloc");
    let export = parse_export_section(&module).into_iter().find(|item| item.name == "romanToInt").expect("公开导出名必须存在");
    assert_eq!(export.index, func_import_count(&module), "导出必须仍指向内部函数本身");
}

/// 负向：物化配方必须与冻结库 ABI 载体合同交叉核对；plan 登记了导出名却没有补齐
/// ABI 载体冻结说明合同缺失，必须在编码前结构化失败，禁止静默合成。
#[test]
fn wasm_library_export_without_frozen_abi_carrier_contract_fails_structured() {
    let error =
        lower_library(utf8_length_call_main(Some(UTF8_TEXT_LAYOUT_ID)), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()), false)
            .expect_err("缺少冻结 ABI 载体合同的库导出物化必须结构化失败");
    assert!(error.contains("缺少冻结 ABI 载体合同"), "错误必须指明缺失的 ABI 载体合同: {error}");
}
