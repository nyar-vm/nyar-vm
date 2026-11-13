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
const UTF16: NyarType = NyarType::Utf16;
const I32: NyarType = NyarType::Integer32 { signed: true };

const UTF8_TEXT_LAYOUT_ID: LayoutId = 7;
const NO_BYTES_LAYOUT_ID: LayoutId = 9;
const HOLDER_LAYOUT_ID: LayoutId = 11;

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

/// `main(text: Utf16)`：%1 = `utf16_length(%0)`；return %1。
fn utf16_length_call_main() -> ExecutableFunction {
    let call = Instruction::from_kind(InstructionKind::Call {
        callee: Operand::Item(utf16_length_callee()),
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
        param_types: vec![UTF16],
        value_types: BTreeMap::from([(ValueRef(0), UTF16), (ValueRef(1), I32)]),
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

fn utf16_length_callee() -> ItemInstanceId {
    ItemInstanceId::from_index(204).expect("测试 callee 身份")
}

/// `env::utf16_length(text: Utf16) -> i32`。
fn utf16_length_import() -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(0).expect("导入下标"),
        link: nyar::ExternalImportLink::host(Some(Identifier::new("wasm")), vec!["env".into(), "utf16_length".into()]),
        parameter_types: vec![UTF16],
        return_type: I32,
        target_operator: None,
    }
}

fn lower(function: ExecutableFunction, layouts: AggregateLayoutPlan) -> nyar_emitter::Result<WasmBinaryModule> {
    lower_with_import(function, layouts, utf8_length_callee(), utf8_length_import())
}

fn lower_with_import(
    function: ExecutableFunction,
    layouts: AggregateLayoutPlan,
    callee: ItemInstanceId,
    import: BackendImport,
) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("host_string_boundary").testing_entry_operation(main_name);
    plan.testing_aggregate_layouts(layouts);
    plan.testing_register_import(callee, import);
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

/// 正向：`WasmJsGlue` ABI 位置的 `utf16` 与 `utf8` 共用 i32 宿主句柄。
/// 宿主导入 `env::utf16_length(text: Utf16) -> i32` 必须编译成 `[i32] -> [i32]`，
/// 且不得合成 `host_intern_utf8`（那是 managed `Utf8Text` 的转换配方）。
/// GC 聚合字段槽仍拒绝 `utf16`，由 `wasm_gc_field_type_byte` 负责。
#[test]
fn wasm_js_glue_utf16_host_import_uses_i32_handle_signature() {
    let module = lower_with_import(
        utf16_length_call_main(),
        layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()),
        utf16_length_callee(),
        utf16_length_import(),
    )
    .expect("JS-glue 边界的 `utf16` 宿主导入必须降为 i32 宿主句柄");
    let import = parse_import_section(&module)
        .into_iter()
        .find(|item| item.module == "env" && item.field == "utf16_length")
        .expect("`env::utf16_length` 导入必须存在");
    assert_eq!(import.kind, WasmExternalKind::Func);
    assert_eq!(
        func_type(&module, import.type_index),
        (vec![WasmValueType::I32], vec![WasmValueType::I32]),
        "`utf16` 形参/返回值必须与 `utf8` 一样是 i32 宿主句柄"
    );
    assert!(
        !import_keys(&module).contains(&("env".to_string(), "host_intern_utf8".to_string())),
        "i32 句柄直通时不得合成 managed 字符串转换导入"
    );
}

fn reference_layout(id: LayoutId, name: &str, fields: Vec<FieldLayout>) -> AggregateLayout {
    AggregateLayout { id, name: name.to_string(), namespace: String::new(), key: None, storage: StorageKind::Reference, size: 4, align: 4, fields }
}

fn holder_and_text_layouts() -> AggregateLayoutPlan {
    AggregateLayoutPlan {
        layouts: vec![
            reference_layout(
                UTF8_TEXT_LAYOUT_ID,
                "Utf8Text",
                vec![FieldLayout { name: "_repr".to_string(), ty: byte_array(), offset: 0, size: 4, align: 4 }],
            ),
            reference_layout(
                HOLDER_LAYOUT_ID,
                "Holder",
                vec![FieldLayout { name: "key".to_string(), ty: UTF8, offset: 0, size: 4, align: 4 }],
            ),
        ],
        ..Default::default()
    }
}

/// `main(holder: Holder)`：`%1 = holder.key`（字段物理字节是 i32 宿主句柄）；
/// `%2 = utf8_length(%1)`；return %2。`bound_layout` 决定字段读结果槽是否携带已登记
/// gc struct layout——即容器字段读形态的触发开关。
fn utf8_field_get_main(bound_layout: Option<LayoutId>) -> ExecutableFunction {
    let holder = NyarType::Nominal { layout: HOLDER_LAYOUT_ID, spelling: Identifier::new("Holder") };
    let tuple_get = Instruction::from_kind(InstructionKind::TupleGet {
        object: Operand::Value(ValueRef(0)),
        layout_id: HOLDER_LAYOUT_ID,
        index: 0,
    });
    let call = Instruction::from_kind(InstructionKind::Call {
        callee: Operand::Item(utf8_length_callee()),
        arguments: vec![Operand::Value(ValueRef(1))],
    });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0)],
        instructions: vec![Instruction { results: vec![ValueRef(1)], ..tuple_get }, Instruction { results: vec![ValueRef(2)], ..call }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(2))) },
    };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: I32,
        param_types: vec![holder.clone()],
        value_types: BTreeMap::from([(ValueRef(0), holder), (ValueRef(1), UTF8), (ValueRef(2), I32)]),
        value_layouts: bound_layout.map(|id| BTreeMap::from([(ValueRef(1), id)])).unwrap_or_default(),
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

/// 正向：聚合字段读（`TupleGet`）与数组元素读同属宿主 i32 句柄载体，字段结果槽绑定
/// `Utf8Text` layout 时必须在目标计划阶段冻结物化配方，并按需合成
/// `env::host_utf8_extract`。修复前该形态在编码期以「缺少物化配方」结构化失败。
#[test]
fn wasm_utf8_field_get_boundary_freezes_extract_recipe() {
    let module = lower(utf8_field_get_main(Some(UTF8_TEXT_LAYOUT_ID)), holder_and_text_layouts())
        .expect("utf8 字段读命中宿主 i32 句柄边界时必须冻结物化配方");
    let imports = import_keys(&module);
    assert!(
        imports.contains(&("env".to_string(), "host_utf8_extract".to_string())),
        "字段读边界必须按需合成 `env::host_utf8_extract`: {imports:?}"
    );
}

/// 按需合成：字段读结果槽未绑定注册 layout 时仍走 i32 句柄直通，不得追加物化导入。
#[test]
fn wasm_utf8_field_get_boundary_without_layout_keeps_i32_handle() {
    let module = lower(utf8_field_get_main(None), holder_and_text_layouts()).expect("未绑定 layout 的字段读必须保持 i32 句柄");
    let imports = import_keys(&module);
    assert!(
        !imports.contains(&("env".to_string(), "host_utf8_extract".to_string())),
        "无 managed 字段读边界时不得合成物化导入: {imports:?}"
    );
}

/// 负向：字段读命中已登记 gc struct layout 但该 layout 缺少整数字节数组字段时，
/// 物化配方无法冻结，必须结构化失败；禁止静默降级或留下运行期 trap。
#[test]
fn wasm_utf8_field_get_layout_without_byte_array_fails_structured() {
    let mut layouts = holder_and_text_layouts();
    layouts.layouts.extend(layout_plan(NO_BYTES_LAYOUT_ID, "HostHandleText", long_array()).layouts);
    let error = lower(utf8_field_get_main(Some(NO_BYTES_LAYOUT_ID)), layouts)
        .expect_err("缺少整数字节数组字段的字段读边界必须在冻结期结构化失败");
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
    plan.testing_export_names_mut().insert(instance, "romanToInt".to_string());
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

/// 库导出 ABI 是 Wasm lane 的私有事实：公共计划不再预冻结物理载体，Wasm 在需要
/// 导出 ABI 时按冻结签名现场派生；未预冻结本身不是错误，物化配方仍须交叉核对。
#[test]
fn wasm_library_export_derives_abi_carrier_without_prefreeze() {
    let module = lower_library(utf8_length_call_main(Some(UTF8_TEXT_LAYOUT_ID)), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()), false)
        .expect("Wasm lane 必须按需派生自己的导出 ABI 载体");
    let imports = import_keys(&module);
    assert!(
        imports.contains(&("env".to_string(), "host_utf8_extract".to_string())),
        "派生出的 i32 载体与 managed 形参槽必须触发宿主句柄物化: {imports:?}"
    );
    assert!(parse_export_section(&module).iter().any(|item| item.name == "romanToInt"), "公开导出名必须存在");
}

fn utf8_concat_callee() -> ItemInstanceId {
    ItemInstanceId::from_index(204).expect("测试 callee 身份")
}

/// provider micro 消费的宿主导入：`env::utf8_concat(a: Utf8, b: Utf8) -> Utf8`。
/// import ABI 按 `wasm_import_type_for_field` 编译为 `[i32, i32] -> [i32]` 宿主句柄签名，
/// 返回值在 managed 字符串槽上必须再物化为 anyref（崩溃现场 `host_concat` 的形状）。
fn utf8_concat_import() -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(1).expect("导入下标"),
        link: nyar::ExternalImportLink::host(Some(Identifier::new("wasm")), vec!["env".into(), "utf8_concat".into()]),
        parameter_types: vec![UTF8, UTF8],
        return_type: UTF8,
        target_operator: None,
    }
}

/// `host_concat(value: Utf8, other: Utf8)`：%2 = `utf8_concat(%0, %1)`；return %2。
/// `bound_layout` 决定形参与结果槽是否携带注册 gc struct layout。
fn utf8_concat_call_main(bound_layout: Option<LayoutId>) -> ExecutableFunction {
    let call = Instruction::from_kind(InstructionKind::Call {
        callee: Operand::Item(utf8_concat_callee()),
        arguments: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(1))],
    });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0), ValueRef(1)],
        instructions: vec![Instruction { results: vec![ValueRef(2)], ..call }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(2))) },
    };
    ExecutableFunction {
        symbol: "host_concat".to_string(),
        return_type: UTF8,
        param_types: vec![UTF8, UTF8],
        value_types: BTreeMap::from([(ValueRef(0), UTF8), (ValueRef(1), UTF8), (ValueRef(2), UTF8)]),
        value_layouts: bound_layout
            .map(|id| BTreeMap::from([(ValueRef(0), id), (ValueRef(1), id), (ValueRef(2), id)]))
            .unwrap_or_default(),
        return_layout: bound_layout,
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

fn lower_concat(function: ExecutableFunction, layouts: AggregateLayoutPlan) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("host_concat")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("host_string_boundary").testing_entry_operation(main_name);
    plan.testing_aggregate_layouts(layouts);
    plan.testing_register_import(utf8_concat_callee(), utf8_concat_import());
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "host_concat")
}

/// 正向：宿主导入按冻结 ABI 返回 i32 句柄、结果 SSA 槽绑定 `Utf8Text` layout（anyref）
/// 时，编码期必须消费冻结的句柄→managed 物化配方，槽位自检不得冲突。
/// 修复前此处 `WASM 编码结果与预规划槽冲突: %2` 进程级 abort（`left: 110 right: 127`）。
#[test]
fn wasm_host_import_utf8_return_materializes_into_managed_slot() {
    let module = lower_concat(utf8_concat_call_main(Some(UTF8_TEXT_LAYOUT_ID)), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()))
        .expect("宿主导入返回值命中冻结物化配方时必须成功");
    let imports = import_keys(&module);
    assert!(imports.contains(&("env".to_string(), "utf8_concat".to_string())), "原宿主导入必须保留: {imports:?}");
    assert!(
        imports.contains(&("env".to_string(), "host_utf8_extract".to_string())),
        "宿主导入返回值边界必须按需合成 `env::host_utf8_extract`: {imports:?}"
    );
    let type_indices = function_section_type_indices(&module);
    let (params, results) = func_type(&module, type_indices[0]);
    assert_eq!(params, vec![WasmValueType::AnyRef, WasmValueType::AnyRef], "managed 字符串形参槽必须是 anyref");
    assert_eq!(results, vec![WasmValueType::AnyRef], "宿主导入返回值物化后函数返回槽必须是 anyref");
}

/// 按需：宿主导入返回值未绑定注册 layout 时仍走 i32 句柄 ABI，
/// 不得凭空合成 `env::host_utf8_extract`，也不得把结果槽升级成 anyref。
#[test]
fn wasm_host_import_utf8_return_without_layout_keeps_i32_handle() {
    let module = lower_concat(utf8_concat_call_main(None), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()))
        .expect("未绑定 layout 的宿主导入返回值必须继续走 i32 ABI");
    let imports = import_keys(&module);
    assert!(!imports.contains(&("env".to_string(), "host_utf8_extract".to_string())), "无物化需求时不得合成 extract 导入: {imports:?}");
    let type_indices = function_section_type_indices(&module);
    let (params, results) = func_type(&module, type_indices[0]);
    assert_eq!(params, vec![WasmValueType::I32, WasmValueType::I32]);
    assert_eq!(results, vec![WasmValueType::I32]);
}

/// 负向：宿主导入返回值命中已注册 gc struct layout 但 layout 缺少整数字节数组字段时，
/// 物化配方无法生成，必须在目标计划冻结期结构化失败（禁止 panic 或 i32 降级）。
#[test]
fn wasm_host_import_utf8_return_without_byte_array_field_fails_structured() {
    let error = lower_concat(utf8_concat_call_main(Some(NO_BYTES_LAYOUT_ID)), layout_plan(NO_BYTES_LAYOUT_ID, "HostHandleText", long_array()))
        .expect_err("缺少整数字节数组字段的宿主导入返回值边界必须在冻结期结构化失败");
    assert!(error.contains("缺少整数字节数组字段"), "错误必须指明冻结失败的缺失事实: {error}");
}

/// 返回值侧测试的库导出装配：注册 `utf8_concat` 宿主导入（形参与返回都是 `Utf8`），
/// 其余冻结步骤与 [`lower_library`] 相同。
fn lower_library_concat(
    function: ExecutableFunction,
    layouts: AggregateLayoutPlan,
    freeze_abis: bool,
) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("host_concat")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("host_string_boundary").testing_entry_operation(main_name);
    plan.testing_aggregate_layouts(layouts);
    plan.testing_register_import(utf8_concat_callee(), utf8_concat_import());
    let instance = *plan.instances().first().expect("测试库导出实例");
    plan.testing_export_names_mut().insert(instance, "romanToInt".to_string());
    if freeze_abis {
        plan.testing_freeze_library_export_abis()?;
    }
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "host_concat")
}

/// 返回值侧正向：库导出 `returns: Utf8` 且返回 layout 绑定 `Utf8Text`（物理槽 anyref）时，
/// 导出边界必须与形参侧共用同一张 managed→host 配方表——合成 wrapper、返回槽替换为
/// i32 宿主句柄、按需追加 `env::host_intern_utf8`，导出重定向到 wrapper。
/// 修复前返回 anyref 原样透传，glue 按 i32 句柄解包得到 null-proto struct。
#[test]
fn wasm_library_export_utf8_return_materializes_host_handle_through_synthesized_wrapper() {
    let module =
        lower_library_concat(utf8_concat_call_main(Some(UTF8_TEXT_LAYOUT_ID)), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()), true)
            .expect("库导出 `returns: Utf8` 返回值必须按冻结配方物化为宿主 i32 句柄");
    let imports = import_keys(&module);
    assert!(
        imports.contains(&("env".to_string(), "host_intern_utf8".to_string())),
        "返回物化必须按需合成 `env::host_intern_utf8`: {imports:?}"
    );
    let export = parse_export_section(&module).into_iter().find(|item| item.name == "romanToInt").expect("公开导出名必须存在");
    assert_eq!(export.kind, WasmExternalKind::Func);
    assert_eq!(export.index, func_import_count(&module) + 1, "导出必须重定向到返回物化 wrapper");

    assert_eq!(parse_code_section(&module).len(), 3, "函数体必须是 main + 物化 wrapper + cabi_realloc");
    let type_indices = function_section_type_indices(&module);
    let (main_params, main_results) = func_type(&module, type_indices[0]);
    assert_eq!(main_params, vec![WasmValueType::AnyRef, WasmValueType::AnyRef], "内部函数形参槽必须保持 managed anyref");
    assert_eq!(main_results, vec![WasmValueType::AnyRef], "内部函数返回槽必须保持 managed anyref");
    let (wrapper_params, wrapper_results) = func_type(&module, type_indices[1]);
    assert_eq!(wrapper_params, vec![WasmValueType::I32, WasmValueType::I32], "wrapper 形参槽必须是 i32 宿主句柄");
    assert_eq!(wrapper_results, vec![WasmValueType::I32], "wrapper 返回槽必须是 i32 宿主句柄");
}

/// 返回值侧按需：返回 layout 未绑定注册 gc struct 时物理返回值本就是 i32 句柄，
/// 不得合成 wrapper，也不得追加 `env::host_intern_utf8`。
#[test]
fn wasm_library_export_utf8_return_without_layout_keeps_i32_handle() {
    let module = lower_library_concat(utf8_concat_call_main(None), layout_plan(UTF8_TEXT_LAYOUT_ID, "Utf8Text", byte_array()), true)
        .expect("未绑定 layout 的库导出返回值必须继续走 i32 ABI");
    let imports = import_keys(&module);
    assert!(!imports.contains(&("env".to_string(), "host_intern_utf8".to_string())), "无返回物化需求时不得合成 intern 导入: {imports:?}");
    let export = parse_export_section(&module).into_iter().find(|item| item.name == "romanToInt").expect("公开导出名必须存在");
    assert_eq!(export.index, func_import_count(&module), "导出必须仍指向内部函数本身");
    let type_indices = function_section_type_indices(&module);
    let (_, results) = func_type(&module, type_indices[0]);
    assert_eq!(results, vec![WasmValueType::I32], "未绑定 layout 的字符串返回值必须是 i32 宿主句柄");
}

/// 返回值侧负向：layout 已登记 gc struct 但缺少整数字节数组字段时，managed→host
/// 返回配方无法在冻结期生成，必须结构化失败并点名实例与类型，禁止退回 anyref 透传。
#[test]
fn wasm_library_export_utf8_return_without_byte_array_field_fails_structured() {
    let error =
        lower_library_concat(utf8_concat_call_main(Some(NO_BYTES_LAYOUT_ID)), layout_plan(NO_BYTES_LAYOUT_ID, "HostHandleText", long_array()), true)
            .expect_err("缺少整数字节数组字段的库导出返回值必须在冻结期结构化失败");
    assert!(error.contains("缺少整数字节数组字段"), "错误必须指明冻结失败的缺失事实: {error}");
}