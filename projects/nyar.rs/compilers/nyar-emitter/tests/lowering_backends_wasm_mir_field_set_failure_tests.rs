//! WASM `FieldSet` 计划不一致的结构化失败合同：字段缺少所属 layout、
//! layout 已注册 gc struct 但对象槽不是引用——两种形状都必须在编码期返回
//! `Err`（与 `Cast` 臂同构），禁止 panic 把可执行合同缺陷升级成编译器崩溃。

use acorn_wasm::WasmBinaryModule;
use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldId, FieldLayout, LayoutId, NyarType, StorageKind};
use std::collections::BTreeMap;

const I32: NyarType = NyarType::Integer32 { signed: true };

/// `main()`：%0 = 常量 7（i32 槽）；`FieldSet(object=%0, field, value=%0)`；return %0。
/// 对象槽固定是 i32 标量——用于注入「gc struct 已注册但对象槽非引用」的冲突形状。
fn field_set_main(field: FieldId) -> ExecutableFunction {
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(7), ty: Some(I32) });
    let set =
        Instruction::from_kind(InstructionKind::FieldSet { object: Operand::Value(ValueRef(0)), field, value: Operand::Value(ValueRef(0)) });
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions: vec![Instruction { results: vec![ValueRef(0)], ..load }, Instruction { results: Vec::new(), ..set }],
        terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
    };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: I32,
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), I32)]),
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

fn lower(
    function: ExecutableFunction,
    layouts: Option<AggregateLayoutPlan>,
    field: FieldId,
    layout_id: LayoutId,
) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("field_set_failure").testing_entry_operation(main_name);
    if let Some(layouts) = layouts {
        plan.testing_aggregate_layouts(layouts);
        plan.testing_register_field(field, layout_id, 0);
    }
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main")
}

/// fail-closed：`FieldSet` 引用的字段身份没有登记所属 layout 时结构化失败，
/// 禁止 panic 或按字段名/短名回查补造 layout。
#[test]
fn wasm_field_set_without_field_layout_fails_structured() {
    let field = FieldId::from_index(9).expect("测试字段身份");
    let error = lower(field_set_main(field), None, field, 0).expect_err("字段缺少所属 layout 必须返回 Err，不得 panic");
    assert!(error.contains("FieldSet 缺少 layout"), "错误必须指明字段 layout 事实缺失: {error}");
}

/// fail-closed：layout 已按引用注册 gc struct，但对象槽是 i32 标量槽——
/// 「写入按 gc struct、ABI 槽按 i32」的表示分叉必须在编码期结构化失败，
/// 禁止发射 `ref.cast` 后留给 V8 运行期 `illegal cast`。
#[test]
fn wasm_field_set_gc_struct_with_scalar_object_slot_fails_structured() {
    let layout_id: LayoutId = 0;
    let field = FieldId::from_index(0).expect("测试字段身份");
    let layout = AggregateLayout {
        id: layout_id,
        name: "BoxedPayload".to_string(),
        namespace: String::new(),
        key: None,
        storage: StorageKind::Reference,
        fields: vec![FieldLayout { name: "payload".to_string(), ty: I32 }],
    };
    let error = lower(field_set_main(field), Some(AggregateLayoutPlan { layouts: vec![layout], ..Default::default() }), field, layout_id)
        .expect_err("gc struct 与对象槽表示冲突必须返回 Err，不得 panic");
    assert!(error.contains("已注册 gc struct，但对象槽不是引用"), "错误必须指明表示冲突: {error}");
}
