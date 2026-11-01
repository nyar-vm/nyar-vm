//! Wasm 表示一致性合同：`register_gc_struct_types` 的注册表是对象槽的**唯一权威**。
//!
//! `representation::wasm_param_value_type_for` 与 `type_uses_gc_struct_param` 必须消费
//! 与构造/字段读取点相同的冻结事实：
//! - 值携带已登记 gc struct 的 layout（如 `Utf8Text` 的真实聚合 layout）→ 槽为 anyref，
//!   宿主字符串特判不得反向污染成 i32；
//! - 宿主字符串在未携带注册 layout 时仍走 i32 句柄 ABI（`const_utf8` / `cli_get_*`），
//!   此时字段读取属于计划不一致，结构化失败而不是静默 coerce。
//!
//! 两个方向都禁止按类型名特判；判定只依据 layout 身份与注册表。

use acorn_wasm::WasmBinaryModule;
use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldId, FieldLayout, LayoutId, NyarType, StorageKind};
use std::collections::BTreeMap;

const UTF8: NyarType = NyarType::Utf8;

fn byte_array() -> NyarType {
    NyarType::Array(Box::new(NyarType::Integer8 { signed: false }))
}

const UTF8_TEXT_LAYOUT_ID: LayoutId = 7;

/// `Utf8Text` 的真实聚合 layout：`_repr: [u8]` 是引用字段，layout 按 Reference 注册 gc struct。
fn utf8_text_layout_plan() -> AggregateLayoutPlan {
    AggregateLayoutPlan {
        layouts: vec![AggregateLayout {
            id: UTF8_TEXT_LAYOUT_ID,
            name: "Utf8Text".to_string(),
            namespace: String::new(),
            key: None,
            storage: StorageKind::Reference,
            size: 4,
            align: 4,
            fields: vec![FieldLayout { name: "_repr".to_string(), ty: byte_array(), offset: 0, size: 4, align: 4 }],
        }],
        ..Default::default()
    }
}

/// `main(text: Utf8)`：%1 = `FieldGet(object=%0, field)`；return %1。
/// `bind_layout` 决定入口参数是否携带注册表里的 gc struct layout 身份——
/// 这正是「注册表权威」与「宿主句柄兜底」两条路的唯一差别。
fn utf8_receiver_field_get_main(field: FieldId, bind_layout: bool) -> ExecutableFunction {
    let get = Instruction::from_kind(InstructionKind::FieldGet { object: Operand::Value(ValueRef(0)), field });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0)],
        instructions: vec![Instruction { results: vec![ValueRef(1)], ..get }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
    };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: byte_array(),
        param_types: vec![UTF8],
        value_types: BTreeMap::from([(ValueRef(0), UTF8), (ValueRef(1), byte_array())]),
        value_layouts: if bind_layout { BTreeMap::from([(ValueRef(0), UTF8_TEXT_LAYOUT_ID)]) } else { BTreeMap::new() },
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

fn lower(function: ExecutableFunction, field: FieldId) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("representation_authority").testing_entry_operation(main_name);
    plan.testing_aggregate_layouts(utf8_text_layout_plan());
    plan.testing_register_field(field, UTF8_TEXT_LAYOUT_ID, 0);
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main")
}

/// 注册表优先：`Utf8` 值携带已登记 gc struct 的 `Utf8Text` layout 时，入口参数槽
/// 必须是 anyref，`FieldGet` 的 gc struct 路径与 ABI 闭合，lowering 成功。
/// 修复前宿主字符串特判把槽判成 i32，构造/读取点与 ABI 分叉。
#[test]
fn wasm_registered_gc_struct_layout_authoritative_over_host_string_handle() {
    let field = FieldId::from_index(0).expect("测试字段身份");
    let module = lower(utf8_receiver_field_get_main(field, true), field).expect("注册表已登记 gc struct layout 时对象槽必须为引用并闭合");
    let type_section = module.sections.iter().find(|section| section.id == 1).expect("type 段");
    assert!(!type_section.bytes.is_empty(), "Utf8Text layout 必须注册 structtype 条目");
}

/// 宿主句柄兜底：同为 `Utf8` 值但未携带注册 layout 绑定时，槽仍是 i32 句柄
/// （`const_utf8` ABI 语义保留）；此时对 gc struct layout 的字段读取是计划不一致，
/// 必须结构化失败，禁止把句柄 coerce 成 `ref.null` 后留给运行期 trap。
#[test]
fn wasm_host_string_without_layout_binding_stays_i32_and_field_get_fails_structured() {
    let field = FieldId::from_index(0).expect("测试字段身份");
    let error = lower(utf8_receiver_field_get_main(field, false), field)
        .expect_err("未绑定 layout 的宿主字符串句柄不得被静默当作 gc struct 对象");
    assert!(error.contains("已注册 gc struct，但对象槽不是引用"), "错误必须指明表示冲突: {error}");
}
