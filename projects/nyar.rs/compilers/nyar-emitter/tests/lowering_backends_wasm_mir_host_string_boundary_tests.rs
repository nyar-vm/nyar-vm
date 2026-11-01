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

use acorn_wasm::{WasmBinaryModule, parse_import_section};
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
