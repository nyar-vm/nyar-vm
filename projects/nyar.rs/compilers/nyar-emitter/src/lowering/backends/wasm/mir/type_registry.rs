//! Wasm GC type section registration and type-index maps.
//!
//! Owns struct/array/sum `structtype`/`arraytype` registration and the
//! layout_id / element-key / sum-name → type_index maps used by emit.
//! This is a Wasm **type registry**, not a second MIR — Semantic MIR remains
//! the language type authority; backend only allocates Wasm type indices.
//!
//! Zero-logic extract from the former monolithic lowerer (B4 seam).

use super::{
    ExecutableLoweringContext, LayoutId, MirInstructionKind, MirOperand, NominalInstanceId, NyarType, StorageKind,
    representation::{is_js_glue_host_string_type, mir_storage_for_type, wasm_gc_field_type_byte_for_glue},
};
use crate::lowering::backends::wasm::gc::{wasm_gc_array_type, wasm_gc_struct_type};
use acorn_wasm::{VALTYPE_ANYREF, VALTYPE_I32};
use std::collections::{BTreeMap, BTreeSet};

/// 扫描 MIR：登记 wasm-gc structtype 所需的 layout_id。
///
/// - `StructNew`：仅 `storage == Reference`
/// - `FieldGet` / `FieldSet`：凡带 layout_id 均登记（含 MIR `storage=Value` 但
///   object 已在 anyref 的路径，如 `SwissTableEntry` 作为 sum payload / 数组元素）
/// - `AggregateCopy`：仅 `storage == Reference`。`storage=Value` 的聚合拷贝是
///   `memory.copy` 或 anyref 浅拷贝，不需要也绝不能凭空登记成 structtype：
///   注册表是物理表示的唯一权威，误登记（如标量 `usize`）会让 ABI 把标量槽
///   提升成 anyref，常量发射随即失败。
pub(super) fn collect_mir_reference_layout_ids(ctx: &ExecutableLoweringContext) -> BTreeSet<LayoutId> {
    let mut ids = BTreeSet::new();
    let exec = &ctx.submission.backend_plan;
    for operation in exec.instances() {
        let Some(view) = exec.get_function(&operation)
        else {
            continue;
        };
        for block in &view.function.blocks {
            for instruction in &block.instructions {
                match &instruction.kind {
                    MirInstructionKind::StructNew { nominal, .. } => {
                        if let Some(layout) = ctx.layout_by_nominal(*nominal) {
                            if layout.storage == StorageKind::Reference {
                                ids.insert(layout.id);
                            }
                        }
                    }
                    MirInstructionKind::FieldGet { field, .. } | MirInstructionKind::FieldSet { field, .. } => {
                        if let Some((layout, _, _)) = ctx.field_layout_by_id(*field) {
                            ids.insert(layout.id);
                        }
                    }
                    MirInstructionKind::AggregateCopy { layout_id, .. } => {
                        if let Some(layout) = ctx.layout_by_id(*layout_id) {
                            if layout.storage == StorageKind::Reference {
                                ids.insert(layout.id);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    ids
}

/// 为所有引用类型 layout 注册 wasm-gc `structtype` 条目,返回 layout_id -> type_index 映射。
///
/// 注册条件（满足其一）：
/// - `layout.storage == Reference`
/// - 含引用字段（或 js_glue utf8→anyref）
/// - MIR 以 Reference 使用该 layout（StructNew/Field*/Reference AggregateCopy）
///
/// 纯 Value 且未被 MIR Reference 使用的 layout 仍走线性内存，不注册。
pub(super) fn register_gc_struct_types(
    ctx: &ExecutableLoweringContext,
    type_indices: &mut Vec<Vec<u8>>,
    js_glue_utf8_as_anyref: bool,
) -> BTreeMap<LayoutId, u32> {
    let mir_ref_layouts = collect_mir_reference_layout_ids(ctx);
    let mut map = BTreeMap::new();
    for layout in &ctx.layouts.layouts {
        let has_ref_fields = layout.fields.iter().any(|field| {
            if js_glue_utf8_as_anyref && is_js_glue_host_string_type(&field.ty) {
                return true;
            }
            mir_storage_for_type(ctx, &field.ty, js_glue_utf8_as_anyref) == StorageKind::Reference
        });
        let mir_uses_reference = mir_ref_layouts.contains(&layout.id);
        if layout.storage != StorageKind::Reference && !has_ref_fields && !mir_uses_reference {
            continue;
        }
        let field_types: Vec<u8> =
            layout.fields.iter().map(|field| wasm_gc_field_type_byte_for_glue(&field.ty, js_glue_utf8_as_anyref)).collect();
        let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        type_indices.push(wasm_gc_struct_type(&field_types));
        map.insert(layout.id, type_index);
    }
    map
}

/// 为 heap `[T]` 的 element_type 注册 wasm-gc `arraytype` 条目,返回 element_type 字符串键 -> type_index 映射。
///
/// 收集来源（与 V 侧 `wasm_module_ensure_array_type` 在 NewArr/ArrayGet/ArraySet 上登记同构）：
/// - `ArrayNew` / `ArrayLiteral` 指令的 element_type
/// - 函数 `param_types` / `value_types` 中的 `Array` / `FixedArray` 元素类型
///   （覆盖仅经 ArrayGet 使用、无 ArrayNew 的路径，如 `Utf8Iterator.next` 的 `Integer8`）
///
/// 注意:tuple 走线性内存；FixedArray 在 wasm-gc 强制轨同样需要 arraytype。
///
/// `[utf8]` / `[Utf8Text]` 在 Node 轨必须登记为 `arraytype [i32]`，与 `const_utf8` 返回的
/// i32 句柄一致；若误用 `wasm_gc_field_type_byte`（Named→anyref），`array.new_fixed` 会在
/// V8 校验阶段报 `expected type anyref, found local.get of type i32`（见宿主侧
/// `["clr","jvm","wasm","ovm"]` 目标列表字面量）。
pub(super) fn register_gc_array_types(
    ctx: &ExecutableLoweringContext,
    type_indices: &mut Vec<Vec<u8>>,
    js_glue_utf8_as_anyref: bool,
) -> BTreeMap<String, u32> {
    let mut map = BTreeMap::new();
    let mut ensure = |element_type: &NyarType, type_indices: &mut Vec<Vec<u8>>, map: &mut BTreeMap<String, u32>| {
        let key = wasm_array_element_type_key(element_type);
        if map.contains_key(&key) {
            return;
        }
        let field_type_byte = wasm_gc_field_type_byte_for_glue(element_type, js_glue_utf8_as_anyref);
        let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        type_indices.push(wasm_gc_array_type(field_type_byte));
        eprintln!("[wasm::arraytype-register] key={key} element={element_type:?} field={field_type_byte} index={type_index}");
        map.insert(key, type_index);
    };
    {
        let exec = &ctx.submission.backend_plan;
        for operation in exec.instances() {
            let Some(view) = exec.get_function(&operation)
            else {
                continue;
            };
            for ty in view.function.param_types.iter().chain(view.function.value_types.values()) {
                match ty {
                    NyarType::Array(element) | NyarType::FixedArray { element, .. } => {
                        ensure(element.as_ref(), type_indices, &mut map);
                    }
                    _ => {}
                }
            }
            for block in &view.function.blocks {
                for instruction in &block.instructions {
                    match &instruction.kind {
                        MirInstructionKind::ArrayNew { array_type, .. } | MirInstructionKind::ArrayFromElements { array_type, .. } => {
                            match array_type {
                                NyarType::Array(element) | NyarType::FixedArray { element, .. } => {
                                    ensure(element.as_ref(), type_indices, &mut map);
                                }
                                _ => {}
                            }
                        }
                        MirInstructionKind::ArrayGet { array, .. }
                        | MirInstructionKind::ArraySet { array, .. }
                        | MirInstructionKind::ArrayLength { array }
                        | MirInstructionKind::ArrayPush { array, .. } => {
                            if let MirOperand::Value(receiver) = array {
                                if let Some(NyarType::Array(element) | NyarType::FixedArray { element, .. }) =
                                    view.function.value_types.get(receiver)
                                {
                                    ensure(element.as_ref(), type_indices, &mut map);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    map
}

/// 为 MIR 中实际出现的 sum nominal 注册 wasm-gc `structtype`。
///
/// 物理布局是**后端私有**选择，但必须由计划事实派生，不能按名字特判：
/// - 字段 0 = tag（`SumRepresentation::tag_width` 决定的整数字段，当前只承诺 1 个字）；
/// - 字段 1 = payload，一律 `anyref`（标量载荷由调用点装箱成 `[i32]` / `[i64]`）。
///
/// 每个 nominal identity 单独注册一个 type，使 `ref.cast` 能把「同一 nominal 的
/// 构造与消费」闭合起来；不同 nominal 不会被结构相同而误认。
pub(super) fn register_gc_sum_types(
    ctx: &ExecutableLoweringContext,
    type_indices: &mut Vec<Vec<u8>>,
) -> BTreeMap<NominalInstanceId, u32> {
    let sums = ctx.submission.backend_plan.sum_representations();
    let mut nominals = BTreeSet::new();
    let exec = &ctx.submission.backend_plan;
    for operation in exec.instances() {
        let Some(view) = exec.get_function(&operation)
        else {
            continue;
        };
        for block in &view.function.blocks {
            for instruction in &block.instructions {
                let nominal = match &instruction.kind {
                    MirInstructionKind::SumNew { nominal, .. }
                    | MirInstructionKind::SumPayloadGet { nominal, .. }
                    | MirInstructionKind::SumVariantIs { nominal, .. } => *nominal,
                    _ => continue,
                };
                if sums.contains_key(&nominal) {
                    nominals.insert(nominal);
                }
            }
        }
    }
    let mut map = BTreeMap::new();
    for nominal in nominals {
        let sum = &sums[&nominal];
        assert!(
            sum.tag_width > 0 && sum.tag_width <= 4,
            "WASM sum 布局只承诺 1..=4 字节 tag（nominal={nominal:?} tag_width={}）",
            sum.tag_width
        );
        let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        type_indices.push(wasm_gc_struct_type(&[VALTYPE_I32, VALTYPE_ANYREF]));
        map.insert(nominal, type_index);
    }
    map
}

/// 为 heap array element_type 生成稳定字符串键,用于 `gc_array_type_indices` 查找。
pub(super) fn wasm_array_element_type_key(element_type: &NyarType) -> String {
    format!("{:?}", element_type)
}
