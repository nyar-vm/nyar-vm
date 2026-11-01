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
//!
//! 另含 arraytype 注册对称性锁定（`register_gc_array_types` ↔ `resolve_gc_array_type_index`），
//! 用于复核账本记录的 `ArrayNew missing gc arraytype` 生产缺口在元素类型冻结后的现状。

use acorn_wasm::WasmBinaryModule;
use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{
        ArrayInitialization, Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef,
    },
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
    let error =
        lower(utf8_receiver_field_get_main(field, false), field).expect_err("未绑定 layout 的宿主字符串句柄不得被静默当作 gc struct 对象");
    assert!(error.contains("已注册 gc struct，但对象槽不是引用"), "错误必须指明表示冲突: {error}");
}

/// arraytype 注册对称性锁定：`ArrayNew` 的元素类型由指令自身进入 `register_gc_array_types`，
/// 消费侧 `resolve_gc_array_type_index` 用同一 `wasm_array_element_type_key` 反查，
/// 冻结具体元素类型的计划不可能 miss。账本记录的 `ArrayNew missing gc arraytype`
/// 属 `7f40a234` 规范化 `builtin.array.new` 之前的观测（当时元素是未实例化泛型）。
#[test]
fn wasm_array_new_element_type_self_registers_and_lowers() {
    let array_ty = NyarType::Array(Box::new(NyarType::Integer64 { signed: true }));
    let new_arr = Instruction::from_kind(InstructionKind::ArrayNew {
        array_type: array_ty.clone(),
        length: Operand::Constant(Constant::Int(3)),
        initialization: ArrayInitialization::Default,
    });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions: vec![Instruction { results: vec![ValueRef(0)], ..new_arr }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    };
    let function = ExecutableFunction {
        symbol: "main".to_string(),
        return_type: array_ty,
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), NyarType::Array(Box::new(NyarType::Integer64 { signed: true })))]),
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
    };
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("array_type_registration").testing_entry_operation(main_name);
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main").expect("ArrayNew 元素类型必须由注册侧自行登记并命中 arraytype");
}

// ── 零字段 / 标量载体不得走 `memory.copy` ─────────────────────────────────────

const EMPTY_VALUE_LAYOUT_ID: LayoutId = 9;
const SCALAR_FIELD_LAYOUT_ID: LayoutId = 10;
const PAIR_AGGREGATE_LAYOUT_ID: LayoutId = 11;

fn empty_value_layout() -> AggregateLayout {
    AggregateLayout {
        id: EMPTY_VALUE_LAYOUT_ID,
        name: "usize".to_string(),
        namespace: std::string::String::new(),
        key: None,
        storage: StorageKind::Value,
        size: 4,
        align: 4,
        fields: Vec::new(),
    }
}

fn named_layout_type(layout: LayoutId, spelling: &str) -> NyarType {
    NyarType::Nominal { layout, spelling: Identifier::new(spelling) }
}

fn base_function(symbol: &str, return_type: NyarType, param_types: Vec<NyarType>) -> ExecutableFunction {
    ExecutableFunction {
        symbol: symbol.to_string(),
        return_type,
        param_types,
        value_types: BTreeMap::new(),
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
        blocks: Vec::new(),
        diagnostics: Vec::new(),
    }
}

/// 读取 code 段里的每个函数体（locals 声明 + 指令）。
fn code_bodies(module: &WasmBinaryModule) -> Vec<Vec<u8>> {
    let section = module.sections.iter().find(|section| section.id == 10).expect("code 段");
    let mut cursor = 0usize;
    let count = read_uleb(&section.bytes, &mut cursor);
    let mut bodies = Vec::new();
    for _ in 0..count {
        let size = read_uleb(&section.bytes, &mut cursor) as usize;
        bodies.push(section.bytes[cursor..cursor + size].to_vec());
        cursor += size;
    }
    bodies
}

fn read_uleb(bytes: &[u8], cursor: &mut usize) -> u32 {
    let mut result = 0u32;
    let mut shift = 0u32;
    loop {
        let byte = bytes[*cursor];
        *cursor += 1;
        result |= u32::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return result;
        }
        shift += 7;
    }
}

fn body_contains_memory_copy(body: &[u8]) -> bool {
    body.windows(2).any(|window| window == [0xFC, 0x0A])
}

fn lower_single(function: ExecutableFunction, layouts: AggregateLayoutPlan) -> WasmBinaryModule {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("zero_field_scalar_contract").testing_entry_operation(main_name);
    plan.testing_aggregate_layouts(layouts);
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main").expect("零字段/标量载体 lowering 必须成功")
}

/// 零字段值布局（`[primitive(...)] structure usize { }`）：参数 → 赋值 → 返回。
/// 槽直接持有载体，赋值必须是标量搬运，禁止把载体数值当地址 `memory.copy`。
#[test]
fn wasm_zero_field_value_layout_assignment_is_scalar_copy() {
    let unit_type = named_layout_type(EMPTY_VALUE_LAYOUT_ID, "usize");
    let mut function = base_function("main", unit_type.clone(), vec![unit_type.clone()]);
    function.value_types.insert(ValueRef(0), unit_type.clone());
    function.value_types.insert(ValueRef(1), unit_type);
    function.blocks.push(Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0)],
        instructions: vec![Instruction {
            results: vec![ValueRef(1)],
            ..Instruction::from_kind(InstructionKind::AggregateCopy {
                layout_id: EMPTY_VALUE_LAYOUT_ID,
                source: Operand::Value(ValueRef(0)),
                dest: Operand::Value(ValueRef(1)),
            })
        }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
    });
    let module = lower_single(
        function,
        AggregateLayoutPlan {
            layouts: vec![empty_value_layout()],
            ..Default::default()
        },
    );
    let bodies = code_bodies(&module);
    assert!(!body_contains_memory_copy(&bodies[0]), "零字段值布局赋值不得发射 memory.copy: {:#x?}", bodies[0]);
}

/// 带标量字段的内联聚合：`TupleNew` 写 `i64` 字段必须逐字段 `i64.store`，
/// 不得退化为对源标量值的 `memory.copy`；读取侧沿用同一分类（标量走 `*load`）。
#[test]
fn wasm_scalar_field_store_uses_typed_store_not_aggregate_copy() {
    let aggregate_type = named_layout_type(SCALAR_FIELD_LAYOUT_ID, "ScalarPair");
    let mut function = base_function("main", aggregate_type.clone(), vec![NyarType::Integer64 { signed: true }]);
    function.value_types.insert(ValueRef(0), NyarType::Integer64 { signed: true });
    function.value_types.insert(ValueRef(1), aggregate_type);
    function.blocks.push(Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0)],
        instructions: vec![Instruction {
            results: vec![ValueRef(1)],
            ..Instruction::from_kind(InstructionKind::TupleNew {
                layout_id: SCALAR_FIELD_LAYOUT_ID,
                fields: vec![Operand::Value(ValueRef(0)), Operand::Value(ValueRef(0))],
            })
        }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
    });
    let module = lower_single(
        function,
        AggregateLayoutPlan {
            layouts: vec![AggregateLayout {
                id: SCALAR_FIELD_LAYOUT_ID,
                name: "ScalarPair".to_string(),
                namespace: std::string::String::new(),
                key: None,
                storage: StorageKind::Value,
                size: 16,
                align: 8,
                fields: vec![
                    FieldLayout { name: "left".to_string(), ty: NyarType::Integer64 { signed: true }, offset: 0, size: 8, align: 8 },
                    FieldLayout { name: "right".to_string(), ty: NyarType::Integer64 { signed: true }, offset: 8, size: 8, align: 8 },
                ],
            }],
            ..Default::default()
        },
    );
    let bodies = code_bodies(&module);
    assert!(!body_contains_memory_copy(&bodies[0]), "标量字段写入不得发射 memory.copy: {:#x?}", bodies[0]);
    assert_eq!(
        bodies[0].windows(1).filter(|window| window[0] == 0x37).count(),
        2,
        "两个 i64 字段必须各发一次 i64.store: {:#x?}",
        bodies[0]
    );
}

/// 正面对照：真正带字段的内联聚合之间的赋值仍必须 `memory.copy`，
/// 防止上面的修复把聚合拷贝一并错误降级成标量搬运。
#[test]
fn wasm_field_bearing_aggregate_assignment_still_uses_memory_copy() {
    let pair_type = named_layout_type(PAIR_AGGREGATE_LAYOUT_ID, "Pair");
    let mut function = base_function("main", pair_type.clone(), vec![pair_type.clone()]);
    function.value_types.insert(ValueRef(0), pair_type.clone());
    function.value_types.insert(ValueRef(1), pair_type);
    function.blocks.push(Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: vec![ValueRef(0)],
        instructions: vec![Instruction {
            results: vec![ValueRef(1)],
            ..Instruction::from_kind(InstructionKind::AggregateCopy {
                layout_id: PAIR_AGGREGATE_LAYOUT_ID,
                source: Operand::Value(ValueRef(0)),
                dest: Operand::Value(ValueRef(1)),
            })
        }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
    });
    let module = lower_single(
        function,
        AggregateLayoutPlan {
            layouts: vec![AggregateLayout {
                id: PAIR_AGGREGATE_LAYOUT_ID,
                name: "Pair".to_string(),
                namespace: std::string::String::new(),
                key: None,
                storage: StorageKind::Value,
                size: 16,
                align: 8,
                fields: vec![
                    FieldLayout { name: "left".to_string(), ty: NyarType::Integer64 { signed: true }, offset: 0, size: 8, align: 8 },
                    FieldLayout { name: "right".to_string(), ty: NyarType::Integer64 { signed: true }, offset: 8, size: 8, align: 8 },
                ],
            }],
            ..Default::default()
        },
    );
    let bodies = code_bodies(&module);
    assert!(body_contains_memory_copy(&bodies[0]), "带字段的内联聚合赋值必须 memory.copy: {:#x?}", bodies[0]);
}
