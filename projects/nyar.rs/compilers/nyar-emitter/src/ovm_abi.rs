//! OVM target 的库导出 ABI 合同。
//!
//! 合同全部由 [`TargetExecutionPlan`] 冻结的类型、载体和布局事实派生：结构层与
//! wasm 共用 [`crate::abi_structure`]，本模块只负责 OVM 物理投影（模块布局下标与
//! 无类型槽位），不复制结构推导。
//!
//! 失败闭合是设计的一部分：
//! - 未注册布局、`i128`、`Utf16`、`TraitObject`、未知/不支持结构一律返回错误；
//! - 任何 `Unsupported` 都不会进入 JSON，也不会被降级成“先跑再说”的兜底；
//! - 参数/返回的物理载体完全由本 lane 的结构投影产出；OVM 不读取也不比较
//!   Wasm 的载体表，零参数 / `Void` 边界是合法的空合同。
//!
//! digest 覆盖规范化 JSON 里的 `schema_version` 与全部导出（按逻辑名排序），
//! 同一布局事实在任意导出位置派生同一 digest。

use std::collections::{BTreeMap, BTreeSet};

use acorn_odin::OdinLayout;
use nyar_types::{AggregateLayoutPlan, ItemInstanceId, LayoutId, NominalInstanceId, NyarType, combined_content_hash};
use serde::Serialize;

use crate::{
    ProgramTextRepresentation, Result, TargetExecutionPlan,
    abi_structure::{AbiStructure, AggregateStructure, abi_structure_for_type_with_layouts, canonical_parameter_identities},
    backend_plan_views::ExecutableFunction,
    lowering::shared::executable::slots::ExecutableSlotPlan,
};

// 合同类型定义在格式 crate，编译器与运行时共用同一份 schema；本模块只保留派生。
pub use acorn_odin::abi::{
    OVM_ABI_SCHEMA_VERSION, OVM_ABI_SIDECAR_SUFFIX, OvmAbiContract, OvmAbiExport, OvmAbiField, OvmAbiParam, OvmAbiSidecar, OvmAbiType,
    OvmAbiVariant, OvmOptionBoxing, OvmTextRepresentation, ovm_abi_sidecar_file,
};

// 合同类型（含 `OvmAbiType`）由 `acorn_odin::abi` 定义并在上方重导出。

// `OvmAbiField` / `OvmTextRepresentation` / `OvmAbiVariant` / `OvmOptionBoxing`
// 以及 `OvmAbiSidecar` 同样由 `acorn_odin::abi` 定义。

/// 从冻结计划派生合同。
///
/// `exports` 是 `(实例身份, 逻辑导出名)`；`module_layouts` / `layout_indices`
/// 是 `.odin` 模块已登记的聚合布局表与 canonical `LayoutId` → 模块下标映射；
/// `sum_layout_indices` 是 lowering 已登记的 `NominalInstanceId` → sum 布局下标。
/// ABI 位置引用但模块尚未登记的布局会按冻结字段数追加到表尾，保证合同里
/// 的每个聚合下标都对应一份真实布局。
pub fn derive_contract(
    plan: &TargetExecutionPlan,
    exports: &[(ItemInstanceId, String)],
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
    sum_layout_indices: &BTreeMap<NominalInstanceId, i32>,
) -> Result<OvmAbiContract> {
    let mut ordered = Vec::with_capacity(exports.len());
    let mut seen = BTreeSet::new();
    for (instance, logical_name) in exports {
        if !seen.insert(logical_name.clone()) {
            return Err(format!("库导出名 `{logical_name}` 重复，无法派生 OVM ABI 合同"));
        }
        ordered.push((instance, logical_name.as_str()));
    }
    // 先按逻辑名排序再投影：布局下标分配与 digest 都与导出登记顺序无关。
    ordered.sort_by(|left, right| left.1.cmp(right.1));
    let mut option_box_indices = BTreeMap::new();
    let mut entries = Vec::with_capacity(ordered.len());
    for (instance, logical_name) in ordered {
        let view = plan.get_function(instance).ok_or_else(|| format!("库导出 `{logical_name}` 缺少 Compiler 函数体，无法派生 OVM ABI 合同"))?;
        entries.push(derive_export(
            plan,
            logical_name,
            &view.function,
            module_layouts,
            layout_indices,
            sum_layout_indices,
            &mut option_box_indices,
        )?);
    }
    let digest = contract_digest(&entries)?;
    let contract = OvmAbiContract { schema_version: OVM_ABI_SCHEMA_VERSION, lane: "ovm".to_string(), exports: entries, digest };
    // 合同必须与它所描述的 `.odin` 布局表一致；不一致 fail-closed。
    contract.verify_against_layouts(module_layouts)?;
    Ok(contract)
}

/// `utf8` 槽的文本表示事实来源。
#[derive(Clone, Copy, Debug)]
enum TextSlot {
    /// 入口形参槽；按 `position` 查逐槽冻结事实。
    Parameter(usize),
    /// 返回槽；只认 `return_layout`。
    Return,
    /// 嵌套槽（字段 / 元素 / 载荷）；目标合同显式声明固定为 raw 宿主串，
    /// 与 lowering 侧 `freeze_nyar_text_representations` 的嵌套读取规则同源。
    Nested,
}

/// 入口形参槽的冻结布局身份表。
///
/// 这是「导出槽文本表示」的唯一派生点：OVM lowering 的入口形参表与
/// [`OvmAbiContract`] 都从这里取事实，禁止任何一方另写一份回落，否则合同与
/// 运行时实际布局会分叉。
///
/// 事实来源与 lowering 完全一致：
/// 1. `value_layouts` 按 SSA 值冻结的布局；
/// 2. 折叠后的槽类型 → `aggregate_layout_by_nyar_type`。
///
/// 两处都没有 = 目标计划没有为该槽冻结托管布局；`None` 只有在程序级表示计划
/// 显式声明 raw（[`TargetExecutionPlan::program_text_representation`]）时才是
/// 合法 raw 宿主串事实，否则由 [`Projection::project_text`] 与 lowering 失败闭合。
pub(crate) fn frozen_entry_parameter_layouts(plan: &TargetExecutionPlan, function: &ExecutableFunction) -> Vec<Option<LayoutId>> {
    let slots = ExecutableSlotPlan::plan_nyar(function);
    let entry = function.blocks.iter().find(|block| block.id == function.entry);
    entry
        .map(|block| {
            block
                .parameters
                .iter()
                .map(|parameter| {
                    function.value_layouts.get(parameter).copied().or_else(|| {
                        slots
                            .value_locals
                            .get(parameter)
                            .and_then(|local| slots.local_types.get(usize::from(*local)))
                            .and_then(|ty| plan.aggregate_layout_by_nyar_type().get(ty).copied())
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn derive_export(
    plan: &TargetExecutionPlan,
    logical_name: &str,
    function: &ExecutableFunction,
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
    sum_layout_indices: &BTreeMap<NominalInstanceId, i32>,
    option_box_indices: &mut BTreeMap<String, i32>,
) -> Result<OvmAbiExport> {
    let identities = canonical_parameter_identities(logical_name, function)?;
    if function.param_types.len() != identities.len() {
        return Err(format!("库导出 `{logical_name}` 参数类型数（{}）与参数身份数（{}）不一致", function.param_types.len(), identities.len()));
    }
    let entry_layouts = frozen_entry_parameter_layouts(plan, function);
    let mut projection =
        Projection { plan, function, entry_layouts: &entry_layouts, module_layouts, layout_indices, sum_layout_indices, option_box_indices };
    let mut params = Vec::with_capacity(identities.len());
    for (position, (_, name)) in identities.iter().enumerate() {
        let ty = function.param_types.get(position).ok_or_else(|| format!("库导出 `{logical_name}` 参数 {position} 缺少类型事实"))?;
        let structure = abi_structure_for_type_with_layouts(
            ty,
            plan.aggregate_layouts(),
            plan.aggregate_layout_by_nyar_type(),
            plan.sum_layout_by_nyar_type(),
        );
        let projected = projection
            .project(&structure, TextSlot::Parameter(position))
            .map_err(|reason| format!("库导出 `{logical_name}` 参数 `{name}` 没有 OVM ABI 合同: {reason}"))?;
        params.push(OvmAbiParam { index: position, name: name.to_string(), carrier: carrier_label(&projected).to_string(), ty: projected });
    }
    let return_structure = abi_structure_for_type_with_layouts(
        &function.return_type,
        plan.aggregate_layouts(),
        plan.aggregate_layout_by_nyar_type(),
        plan.sum_layout_by_nyar_type(),
    );
    let returns = projection
        .project(&return_structure, TextSlot::Return)
        .map_err(|reason| format!("库导出 `{logical_name}` 返回类型没有 OVM ABI 合同: {reason}"))?;
    Ok(OvmAbiExport { logical_name: logical_name.to_string(), params, returns_carrier: carrier_label(&returns).to_string(), returns })
}

/// 结构 → OVM 物理投影。
///
/// `Unsupported` 与未注册布局都在这里结构化失败；投影成功的结果不含任何
/// “未知”分支，因此宿主不需要也不允许再猜。
struct Projection<'a> {
    plan: &'a TargetExecutionPlan,
    function: &'a ExecutableFunction,
    /// 入口参数逐槽冻结布局（与 lowering 同源）。
    entry_layouts: &'a [Option<LayoutId>],
    module_layouts: &'a mut Vec<OdinLayout>,
    layout_indices: &'a mut BTreeMap<LayoutId, i32>,
    sum_layout_indices: &'a BTreeMap<NominalInstanceId, i32>,
    /// 显式 Option box 布局的跨导出去重表（规范化 inner 身份 → 模块下标）。
    option_box_indices: &'a mut BTreeMap<String, i32>,
}

impl Projection<'_> {
    fn project(&mut self, structure: &AbiStructure, slot: TextSlot) -> Result<OvmAbiType> {
        Ok(match structure {
            AbiStructure::Unit => OvmAbiType::Unit,
            AbiStructure::I32 => OvmAbiType::I32,
            AbiStructure::I64 => OvmAbiType::I64,
            AbiStructure::F32 => OvmAbiType::F32,
            AbiStructure::F64 => OvmAbiType::F64,
            AbiStructure::Bool => OvmAbiType::Bool,
            AbiStructure::Char => OvmAbiType::Char,
            AbiStructure::Utf8 => self.project_text(slot)?,
            AbiStructure::Array { element, .. } => OvmAbiType::Array { element: Box::new(self.project(element, TextSlot::Nested)?) },
            AbiStructure::FixedArray { element, length } => {
                OvmAbiType::FixedArray { element: Box::new(self.project(element, TextSlot::Nested)?), length: *length }
            }
            AbiStructure::Option { inner } => {
                let inner = self.project(inner, TextSlot::Nested)?;
                // 内层也是 Option 时，`null` 同时表示外层 None 与内层 None：
                // 必须显式 box，否则 `Some(None)` 与外层 `None` 塌陷成同一物理值。
                let boxing = if matches!(inner, OvmAbiType::Option { .. }) {
                    OvmOptionBoxing::Boxed { layout: self.option_box_layout(&inner)?, tag_slot: 0, payload_slot: 1 }
                }
                else {
                    OvmOptionBoxing::Niche
                };
                OvmAbiType::Option { inner: Box::new(inner), boxing }
            }
            AbiStructure::Tuple { fields } => {
                OvmAbiType::Tuple { fields: fields.iter().map(|field| self.project(field, TextSlot::Nested)).collect::<Result<Vec<_>>>()? }
            }
            AbiStructure::Aggregate(aggregate) => self.project_aggregate(aggregate)?,
            AbiStructure::InstanceHandle { layout, name, module_path } => {
                let aggregate = self
                    .plan
                    .aggregate_layouts()
                    .layouts
                    .iter()
                    .find(|item| item.id == *layout)
                    .ok_or_else(|| format!("layout `{layout}` 未注册，无法投影引用实例 `{module_path}::{name}`"))?;
                OvmAbiType::InstanceHandle {
                    layout: layout_index(*layout, aggregate.fields.len(), self.module_layouts, self.layout_indices)?,
                    name: name.clone(),
                    module_path: module_path.clone(),
                }
            }
            AbiStructure::Unite { nominal, name, variants, payloads, .. } => {
                // 物理 sum 布局由 lowering 已登记的 nominal → 模块下标映射给出；
                // 缺失即失败闭合，绝不在合同里留“未知布局”。
                let layout = *self
                    .sum_layout_indices
                    .get(nominal)
                    .ok_or_else(|| format!("sum nominal `{nominal:?}` 没有已登记的 OVM 模块布局，拒绝按未知布局解释内存"))?;
                let variants = variants
                    .iter()
                    .zip(payloads.iter())
                    .map(|(variant, payload)| {
                        let payload = payload.as_ref().map(|payload| self.project(payload, TextSlot::Nested)).transpose()?.map(Box::new);
                        Ok(OvmAbiVariant { name: variant.name.clone(), tag: variant.tag, payload })
                    })
                    .collect::<Result<Vec<_>>>()?;
                OvmAbiType::Unite { layout, name: name.clone(), tag_slot: 0, payload_slot: 1, variants }
            }
            AbiStructure::Unsupported { reason } => return Err(reason.clone()),
        })
    }

    /// `utf8` 文本边界投影。
    ///
    /// 每个槽的表示都是显式事实：
    /// - 入口参数只认 [`frozen_entry_parameter_layouts`] 的逐槽结果，且与 lowering
    ///   同源；该槽解析不到托管布局时，只有程序级表示计划显式声明 raw 才是合法
    ///   raw 串，否则说明合同与运行时布局分叉，直接失败闭合；
    /// - 返回槽只认 `return_layout`（lowering 要求 `utf8` 返回必须是托管对象）；
    /// - 嵌套槽没有逐槽事实，显式落到 raw 宿主串，绝不回查全局布局猜测。
    fn project_text(&mut self, slot: TextSlot) -> Result<OvmAbiType> {
        let layout_id = match slot {
            TextSlot::Parameter(position) => {
                let layout = self.entry_layouts.get(position).copied().flatten();
                if layout.is_none() && self.plan.program_text_representation() != Some(ProgramTextRepresentation::Raw) {
                    return Err(format!(
                        "导出参数 #{position} 的 `utf8` 槽没有解析到目标计划已冻结的文本布局：\
                         程序级表示计划没有显式声明 raw 表示，缺登记不是「程序选择 raw」的证据，拒绝按 raw 串解释"
                    ));
                }
                layout
            }
            TextSlot::Return => Some(self.function.return_layout.ok_or_else(|| {
                "`utf8` 返回值没有目标计划冻结的 `return_layout`：OVM lowering 要求返回槽是托管文本对象，拒绝按 raw 串解释".to_string()
            })?),
            TextSlot::Nested => None,
        };
        let Some(layout_id) = layout_id
        else {
            return Ok(OvmAbiType::Text { representation: OvmTextRepresentation::Raw });
        };
        let layout = find_layout(self.plan.aggregate_layouts(), layout_id)?;
        let is_byte_slice = matches!(
            layout.fields.as_slice(),
            [field]
                if matches!(
                    &field.ty,
                    NyarType::Array(element) | NyarType::FixedArray { element, .. }
                        if matches!(element.as_ref(), NyarType::Integer8 { signed: false })
                )
        );
        if !is_byte_slice {
            let shape = layout.fields.iter().map(|field| format!("{}:{}", field.name, field.ty)).collect::<Vec<_>>().join(", ");
            return Err(format!(
                "OVM 文本载体 layout `{}`（id={:?}）缺少冻结的单一 `[u8]` 槽配方：实际 fields=[{shape}]",
                layout.name, layout.id
            ));
        }
        Ok(OvmAbiType::Text {
            representation: OvmTextRepresentation::Managed {
                layout: layout_index(layout_id, layout.fields.len(), self.module_layouts, self.layout_indices)?,
                field_count: layout.fields.len(),
                byte_slot: 0,
            },
        })
    }

    /// 为嵌套 Option 登记显式 2 槽 box 布局；同一 inner 形状跨导出复用同一下标。
    fn option_box_layout(&mut self, inner: &OvmAbiType) -> Result<i32> {
        let key = serde_json::to_string(inner).map_err(|error| format!("Option box 身份无法规范化序列化: {error}"))?;
        if let Some(index) = self.option_box_indices.get(&key) {
            return Ok(*index);
        }
        let index = i32::try_from(self.module_layouts.len()).map_err(|_| "OVM 模块布局表下标溢出，无法登记 Option box".to_string())?;
        self.module_layouts.push(OdinLayout { field_count: 2 });
        self.option_box_indices.insert(key, index);
        Ok(index)
    }

    fn project_aggregate(&mut self, aggregate: &AggregateStructure) -> Result<OvmAbiType> {
        let layout_id = aggregate
            .layout
            .ok_or_else(|| format!("聚合 `{}::{}` 没有冻结布局身份，OVM 拒绝按身份猜字段", aggregate.module_path, aggregate.name))?;
        let layout = find_layout(self.plan.aggregate_layouts(), layout_id)?;
        if layout.storage.is_reference() {
            return Err(format!("聚合 `{}::{}` 是引用存储，必须投影为共享身份而不是字段快照", layout.namespace, layout.name));
        }
        let namespace = layout.namespace.clone();
        let layout_name = layout.name.clone();
        let mut fields = Vec::with_capacity(aggregate.fields.len());
        for field in &aggregate.fields {
            fields.push(OvmAbiField {
                name: field.name.clone(),
                ty: self
                    .project(&field.ty, TextSlot::Nested)
                    .map_err(|reason| format!("聚合 `{namespace}::{layout_name}` 字段 `{}` 没有 OVM ABI 合同: {reason}", field.name))?,
            });
        }
        if fields.len() != layout.fields.len() {
            return Err(format!(
                "聚合 `{namespace}::{layout_name}` 的合同字段数（{}）与冻结布局字段数（{}）不一致",
                fields.len(),
                layout.fields.len()
            ));
        }
        Ok(OvmAbiType::Aggregate {
            layout: layout_index(layout_id, layout.fields.len(), self.module_layouts, self.layout_indices)?,
            name: aggregate.name.clone(),
            module_path: aggregate.module_path.clone(),
            fields,
        })
    }
}

fn find_layout(plan: &AggregateLayoutPlan, layout_id: LayoutId) -> Result<&nyar_types::AggregateLayout> {
    plan.layouts.iter().find(|item| item.id == layout_id).ok_or_else(|| format!("layout `{layout_id}` 未注册，无法派生 OVM 物理投影"))
}

/// 把 canonical 布局身份解析为 `.odin` 模块布局下标；尚未登记的布局按冻结
/// 字段数追加到表尾，绝不复用其它布局的下标。
fn layout_index(
    layout_id: LayoutId,
    field_count: usize,
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
) -> Result<i32> {
    if let Some(index) = layout_indices.get(&layout_id) {
        return Ok(*index);
    }
    let field_count = i32::try_from(field_count).map_err(|_| format!("layout `{layout_id}` 字段数 `{field_count}` 超出 OVM 槽位范围"))?;
    let index = i32::try_from(module_layouts.len()).map_err(|_| format!("OVM 模块布局表下标溢出，无法登记 layout `{layout_id}`"))?;
    module_layouts.push(OdinLayout { field_count });
    layout_indices.insert(layout_id, index);
    Ok(index)
}

/// 载体的 JSON 标签。
///
/// OVM 只输出本 lane 的投影标签：raw 宿主串标 `text`，托管文本对象标 `handle`。
/// 这里不读取、也不比较 Wasm 的载体表。
fn carrier_label(ty: &OvmAbiType) -> &'static str {
    match ty {
        OvmAbiType::Text { representation: OvmTextRepresentation::Raw } => "text",
        OvmAbiType::Text { representation: OvmTextRepresentation::Managed { .. } } => "handle",
        OvmAbiType::Unit => "void",
        OvmAbiType::I32 | OvmAbiType::Bool | OvmAbiType::Char => "i32",
        OvmAbiType::I64 => "i64",
        OvmAbiType::F32 => "f32",
        OvmAbiType::F64 => "f64",
        OvmAbiType::Array { .. }
        | OvmAbiType::FixedArray { .. }
        | OvmAbiType::Option { .. }
        | OvmAbiType::Tuple { .. }
        | OvmAbiType::Aggregate { .. }
        | OvmAbiType::InstanceHandle { .. }
        | OvmAbiType::Unite { .. } => "handle",
    }
}

/// 合同 digest：覆盖 schema 版本与按逻辑名排序的导出事实。
fn contract_digest(exports: &[OvmAbiExport]) -> Result<String> {
    #[derive(Serialize)]
    struct DigestInput<'a> {
        schema_version: u32,
        lane: &'a str,
        exports: &'a [OvmAbiExport],
    }
    let canonical = serde_json::to_string(&DigestInput { schema_version: OVM_ABI_SCHEMA_VERSION, lane: "ovm", exports })
        .map_err(|error| format!("OVM ABI 合同无法规范化序列化: {error}"))?;
    Ok(format!("sha256-{}", combined_content_hash(&[&canonical])))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use nyar::{Identifier, QualifiedName};
    use nyar_types::{AggregateLayout, FieldLayout, StorageKind, ValueOrigin};

    use super::*;
    use crate::contracts::{Block, BlockRef, Terminator, Value, ValueRef};

    fn nyar(s: &str) -> NyarType {
        crate::test_canonical_primitive(s)
    }

    #[allow(deprecated)]
    fn function(parameters: &[(usize, &str, NyarType)], return_type: NyarType) -> ExecutableFunction {
        let parameter_values =
            parameters.iter().map(|(index, _, _)| ValueRef(u32::try_from(*index).expect("测试参数下标"))).collect::<Vec<_>>();
        ExecutableFunction {
            symbol: "exported".to_string(),
            return_type,
            param_types: parameters.iter().map(|(_, _, ty)| ty.clone()).collect(),
            value_types: parameters.iter().map(|(index, _, ty)| (ValueRef(u32::try_from(*index).expect("测试参数下标")), ty.clone())).collect(),
            value_layouts: BTreeMap::new(),
            return_layout: None,
            entry: BlockRef(0),
            values: parameters
                .iter()
                .map(|(index, name, _)| Value {
                    id: ValueRef(u32::try_from(*index).expect("测试参数下标")),
                    origin: ValueOrigin::Parameter { index: *index, name: (*name).to_string() },
                })
                .collect(),
            suspend_points: Vec::new(),
            frame_layouts: Vec::new(),
            continuations: Vec::new(),
            case_chains: Vec::new(),
            state_machine: None,
            suspend_plan: None,
            blocks: vec![Block {
                id: BlockRef(0),
                label: "entry".to_string(),
                parameters: parameter_values,
                instructions: Vec::new(),
                terminator: Terminator::Return { value: None },
            }],
            diagnostics: Vec::new(),
        }
    }

    fn plan_with(export: &str, function: ExecutableFunction) -> TargetExecutionPlan {
        let name = QualifiedName::new(vec![Identifier::new(export)]);
        let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(name, function)]));
        for instance in plan.instances() {
            plan.testing_export_names_mut().insert(instance, export.to_string());
        }
        plan
    }

    fn plan(export: &str, function: ExecutableFunction) -> TargetExecutionPlan {
        plan_with(export, function)
    }

    fn plan_multi(entries: &[(&str, ExecutableFunction)]) -> TargetExecutionPlan {
        let mut names = entries.iter().map(|(name, _)| *name).collect::<Vec<_>>();
        names.sort_unstable();
        let functions = entries
            .iter()
            .map(|(name, function)| (QualifiedName::new(vec![Identifier::new(name)]), function.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut plan = TargetExecutionPlan::from_functions(functions);
        for (instance, name) in plan.instances().into_iter().zip(names) {
            plan.testing_export_names_mut().insert(instance, name.to_string());
        }
        plan
    }

    fn export_entries(plan: &TargetExecutionPlan) -> Vec<(ItemInstanceId, String)> {
        plan.instances()
            .into_iter()
            .map(|instance| {
                let name = plan.abi_name_for_instance(instance).map(|name| name.to_string()).unwrap_or_else(|| "exported".to_string());
                (instance, name)
            })
            .collect()
    }

    fn derive_with_layouts(plan: &TargetExecutionPlan) -> Result<(OvmAbiContract, Vec<OdinLayout>)> {
        let mut module_layouts = Vec::new();
        let mut layout_indices = BTreeMap::new();
        // 生产 lowering 先登记 unite 的 `[tag, payload]` 布局；测试里按 nominal
        // 一一补上 2 槽布局，再交给合同派生，保证 `verify_against_layouts` 有真布局可查。
        let mut sum_layout_indices = BTreeMap::new();
        for sum in plan.sum_layout_by_nyar_type().values() {
            if sum_layout_indices.contains_key(&sum.nominal) {
                continue;
            }
            let index = i32::try_from(module_layouts.len()).expect("测试布局下标");
            module_layouts.push(OdinLayout { field_count: 2 });
            sum_layout_indices.insert(sum.nominal, index);
        }
        let contract = derive_contract(plan, &export_entries(plan), &mut module_layouts, &mut layout_indices, &sum_layout_indices)?;
        Ok((contract, module_layouts))
    }

    fn derive(plan: &TargetExecutionPlan) -> Result<OvmAbiContract> {
        derive_with_layouts(plan).map(|(contract, _)| contract)
    }

    fn layout(id: LayoutId, name: &str, storage: StorageKind, fields: Vec<(&str, NyarType)>) -> AggregateLayout {
        AggregateLayout {
            id,
            name: name.to_string(),
            namespace: "demo".to_string(),
            key: None,
            storage,
            fields: fields.into_iter().map(|(name, ty)| FieldLayout { name: name.to_string(), ty }).collect(),
        }
    }

    fn export_of(contract: &OvmAbiContract) -> &OvmAbiExport {
        contract.exports.first().expect("至少一个导出")
    }

    fn export_of_mut(contract: &mut OvmAbiContract) -> &mut OvmAbiExport {
        contract.exports.first_mut().expect("至少一个导出")
    }

    #[test]
    fn heap_array_of_i64_projects_elementwise() {
        let ty = NyarType::Array(Box::new(nyar("i64")));
        let plan = plan("take", function(&[(0, "values", ty.clone())], ty));
        let contract = derive(&plan).expect("`[i64]` 必须有 OVM 合同");
        let export = export_of(&contract);
        assert_eq!(export.params[0].ty, OvmAbiType::Array { element: Box::new(OvmAbiType::I64) });
        assert_eq!(export.params[0].carrier, "handle");
        assert_eq!(export.returns, OvmAbiType::Array { element: Box::new(OvmAbiType::I64) });
        assert!(contract.digest.starts_with("sha256-"), "digest: {}", contract.digest);
    }

    #[test]
    fn named_struct_projects_layout_slots() {
        let ty = NyarType::Nominal { layout: 7, spelling: Identifier::new("demo::Point") };
        let mut plan = plan("point", function(&[(0, "point", ty.clone())], ty.clone()));
        plan.testing_register_type_layout(ty, layout(7, "Point", StorageKind::Value, vec![("x", nyar("i64")), ("y", nyar("i64"))]));
        let contract = derive(&plan).expect("具名聚合必须有 OVM 合同");
        let OvmAbiType::Aggregate { layout: index, name, module_path, fields } = &export_of(&contract).params[0].ty
        else {
            panic!("具名 struct 必须投影为 Aggregate")
        };
        assert_eq!(*index, 0, "首个 ABI 布局登记为下标 0");
        assert_eq!(name, "Point");
        assert_eq!(module_path, "demo");
        assert_eq!(
            fields.iter().map(|field| (field.name.as_str(), field.ty.clone())).collect::<Vec<_>>(),
            vec![("x", OvmAbiType::I64), ("y", OvmAbiType::I64)]
        );
    }

    #[test]
    fn nested_aggregate_projects_recursively() {
        let inner_ty = NyarType::Nominal { layout: 8, spelling: Identifier::new("demo::Inner") };
        let outer_ty = NyarType::Nominal { layout: 7, spelling: Identifier::new("demo::Outer") };
        let mut plan = plan("outer", function(&[(0, "value", outer_ty.clone())], outer_ty.clone()));
        plan.testing_register_type_layout(
            inner_ty.clone(),
            layout(8, "Inner", StorageKind::Value, vec![("value", nyar("i64")), ("tag", nyar("i32"))]),
        );
        plan.testing_register_type_layout(outer_ty, layout(7, "Outer", StorageKind::Value, vec![("inner", inner_ty), ("count", nyar("i32"))]));
        let contract = derive(&plan).expect("嵌套聚合必须有 OVM 合同");
        let OvmAbiType::Aggregate { fields, .. } = &export_of(&contract).params[0].ty
        else {
            panic!("外层必须是 Aggregate")
        };
        let OvmAbiType::Aggregate { layout: inner_index, fields: inner_fields, .. } = &fields[0].ty
        else {
            panic!("嵌套字段必须是 Aggregate")
        };
        assert_eq!(*inner_index, 0);
        assert_eq!(inner_fields.len(), 2);
        assert_eq!(fields[1].ty, OvmAbiType::I32);
    }

    #[test]
    fn generic_reference_instance_keeps_identity_despite_array_and_integer_fields() {
        let list = NyarType::Apply(Box::new(NyarType::Named(Identifier::new("std::collections::ArrayList"))), vec![nyar("i64")]);
        let mut plan = plan("list", function(&[(0, "values", list.clone())], list.clone()));
        plan.testing_register_type_layout(
            list,
            layout(9, "ArrayList", StorageKind::Reference, vec![("_items", NyarType::Array(Box::new(nyar("i64")))), ("_length", nyar("i32"))]),
        );
        let contract = derive(&plan).expect("泛型实例必须有 OVM 合同");
        let OvmAbiType::InstanceHandle { layout: index, name, .. } = &export_of(&contract).params[0].ty
        else {
            panic!("引用存储实例必须保留 InstanceHandle 身份")
        };
        assert_eq!(*index, 0);
        assert_eq!(name, "ArrayList");
        assert_eq!(export_of(&contract).returns, export_of(&contract).params[0].ty);
    }

    #[test]
    fn raw_utf8_parameter_uses_host_text_carrier() {
        let ty = nyar("utf8");
        let mut plan = plan("greet", function(&[(0, "name", ty)], nyar("unit")));
        // raw 槽的合法性来自程序级表示计划的显式声明，不是布局表为空的反推。
        plan.freeze_program_text_representation(ProgramTextRepresentation::Raw);
        let contract = derive(&plan).expect("`utf8` 必须有 OVM 合同");
        let export = export_of(&contract);
        assert_eq!(export.params[0].ty, OvmAbiType::Text { representation: OvmTextRepresentation::Raw });
        assert_eq!(export.params[0].carrier, "text");
        assert_eq!(export.returns, OvmAbiType::Unit);
        assert_eq!(export.returns_carrier, "void");
    }

    /// 反例：布局表为空且程序级表示计划未显式声明 raw——缺登记不是「程序选择
    /// raw」的证据，合同派生必须失败闭合，不得把 missing 吸收成 raw 串。
    #[test]
    fn utf8_parameter_without_explicit_raw_fact_fails_closed() {
        let ty = nyar("utf8");
        let plan = plan("greet", function(&[(0, "name", ty)], nyar("unit")));
        let error = derive(&plan).expect_err("布局表为空不得反推 raw，缺程序级显式事实必须失败闭合");
        assert!(error.contains("拒绝按 raw 串解释"), "错误必须点名禁止 raw 吸收，实际: {error}");
        assert!(error.contains("没有显式声明 raw"), "错误必须点名缺失的程序级显式声明，实际: {error}");
    }

    #[test]
    fn utf8_return_without_frozen_layout_fails_closed() {
        let ty = nyar("utf8");
        let mut plan = plan("greet", function(&[(0, "name", ty.clone())], ty));
        // 形参 raw 槽由程序级显式声明授权，让断言聚焦返回槽自己的失败闭合。
        plan.freeze_program_text_representation(ProgramTextRepresentation::Raw);
        let error = derive(&plan).expect_err("`utf8` 返回槽没有 return_layout 必须失败闭合");
        assert!(error.contains("return_layout"), "got: {error}");
    }

    #[test]
    fn raw_and_managed_text_slots_coexist_in_one_contract() {
        let text_ty = nyar("utf8");
        let text_layout = layout(11, "Utf8Text", StorageKind::Reference, vec![("_repr", NyarType::Array(Box::new(nyar("u8"))))]);
        let mut plan = plan_multi(&[
            ("raw", function(&[(0, "name", text_ty.clone())], nyar("unit"))),
            ("managed", function(&[(0, "name", text_ty.clone())], nyar("unit"))),
        ]);
        // 只登记布局表、不登记 `aggregate_layout_by_nyar_type`：托管表示只来自
        // 该槽自己的 `value_layouts` 事实；raw 槽由程序级显式 raw 声明授权，
        // 不是布局表为空的反推。
        plan.freeze_program_text_representation(ProgramTextRepresentation::Raw);
        plan.testing_aggregate_layouts(nyar_types::AggregateLayoutPlan { layouts: vec![text_layout], ..Default::default() });
        for instance in plan.instances() {
            if plan.abi_name_for_instance(instance).map(|name| name.to_string()).as_deref() == Some("managed") {
                plan.testing_functions_mut().get_mut(&instance).expect("函数体").value_layouts.insert(ValueRef(0), 11u32);
            }
        }
        let contract = derive(&plan).expect("同一合同两种文本表示都必须按各自事实解释");
        let raw = contract.export("raw").expect("raw 导出");
        let managed = contract.export("managed").expect("managed 导出");
        assert_eq!(raw.params[0].ty, OvmAbiType::Text { representation: OvmTextRepresentation::Raw });
        assert_eq!(raw.params[0].carrier, "text");
        assert_eq!(
            managed.params[0].ty,
            OvmAbiType::Text { representation: OvmTextRepresentation::Managed { layout: 0, field_count: 1, byte_slot: 0 } }
        );
        assert_eq!(managed.params[0].carrier, "handle");
    }

    #[test]
    fn contract_layout_mismatch_fails_closed() {
        let ty = nyar("utf8");
        let mut plan = plan("greet", function(&[(0, "name", ty.clone())], nyar("unit")));
        plan.testing_register_type_layout(
            ty,
            layout(11, "Utf8Text", StorageKind::Reference, vec![("_repr", NyarType::Array(Box::new(nyar("u8"))))]),
        );
        let (contract, _) = derive_with_layouts(&plan).expect("合同");
        // 模拟 `.odin` 布局表与合同声明不一致：托管文本槽声明 1 槽，产物却是 2 槽。
        let error = contract.verify_against_layouts(&[OdinLayout { field_count: 2 }]).expect_err("合同与运行时布局不一致必须失败闭合");
        assert!(error.contains("不一致"), "got: {error}");
    }

    #[test]
    fn sidecar_hash_covers_whole_artifact() {
        let plan = plan("first", function(&[(0, "left", nyar("i32"))], nyar("i32")));
        let contract = derive(&plan).expect("合同");
        let bytes = b"odin-artifact-bytes";
        let sidecar = OvmAbiSidecar::new(contract, "app.odin".to_string(), bytes);
        assert!(sidecar.artifact_sha256.starts_with("sha256-"), "哈希必须带算法前缀: {}", sidecar.artifact_sha256);
        sidecar.verify_against(bytes).expect("同一份产物字节必须通过");
        let mut tampered = bytes.to_vec();
        tampered.push(0);
        assert!(sidecar.verify_against(&tampered).is_err(), "产物字节改动后必须失败闭合");
    }

    #[test]
    fn option_uses_niche_when_inner_is_not_option() {
        let ty = NyarType::Nullable(Box::new(nyar("i64")));
        let plan = plan("maybe", function(&[(0, "value", ty.clone())], ty));
        let contract = derive(&plan).expect("`i64?` 必须有 OVM 合同");
        assert_eq!(export_of(&contract).params[0].ty, OvmAbiType::Option { inner: Box::new(OvmAbiType::I64), boxing: OvmOptionBoxing::Niche });
    }

    #[test]
    fn nested_option_projects_explicit_box_and_three_shapes_are_distinct() {
        let ty = NyarType::Nullable(Box::new(NyarType::Nullable(Box::new(nyar("i64")))));
        let plan = plan("maybe", function(&[(0, "value", ty.clone())], ty));
        let (contract, _) = derive_with_layouts(&plan).expect("`i64??` 必须有 OVM 合同");
        let returns = &export_of(&contract).returns;
        let OvmAbiType::Option { inner, boxing } = returns
        else {
            panic!("外层必须是 Option")
        };
        let OvmOptionBoxing::Boxed { layout, tag_slot, payload_slot } = *boxing
        else {
            panic!("嵌套 Option 必须显式 box，禁止 Niche 塌陷")
        };
        assert_eq!((layout, tag_slot, payload_slot), (0, 0, 1), "box 的冻结配方");
        assert_eq!(**inner, OvmAbiType::Option { inner: Box::new(OvmAbiType::I64), boxing: OvmOptionBoxing::Niche });
        // 三种形态按合同编码：`None` / `Some(None)` / `Some(Some(x))` 必须两两可区分。
        let shapes = encode_option_shapes(returns);
        assert_ne!(shapes[0], shapes[1], "`None` 与 `Some(None)` 不得塌陷");
        assert_ne!(shapes[1], shapes[2], "`Some(None)` 与 `Some(Some(x))` 不得塌陷");
        assert_ne!(shapes[0], shapes[2], "`None` 与 `Some(Some(x))` 不得塌陷");
    }

    #[test]
    fn collapsed_nested_option_fails_verification() {
        let ty = NyarType::Nullable(Box::new(NyarType::Nullable(Box::new(nyar("i64")))));
        let plan = plan("maybe", function(&[(0, "value", ty.clone())], ty));
        let (mut contract, layouts) = derive_with_layouts(&plan).expect("合同");
        // 人为把外层退回 `Niche`：这正是 `Some(None)` 与 `None` 物理同形的塌陷合同。
        let OvmAbiType::Option { boxing, .. } = &mut export_of_mut(&mut contract).returns
        else {
            panic!("外层必须是 Option")
        };
        *boxing = OvmOptionBoxing::Niche;
        let error = contract.verify_against_layouts(&layouts).expect_err("塌陷合同必须失败闭合");
        assert!(error.contains("塌陷"), "got: {error}");
    }

    /// 按合同编码 `None` / `Some(None)` / `Some(Some(x))`；只看合同，不看运行期类型。
    fn encode_option_shapes(ty: &OvmAbiType) -> [String; 3] {
        let OvmAbiType::Option { boxing, .. } = ty
        else {
            panic!("必须是 Option")
        };
        match boxing {
            OvmOptionBoxing::Niche => ["null".to_string(), "null".to_string(), "7".to_string()],
            OvmOptionBoxing::Boxed { layout, tag_slot, payload_slot } => [
                "null".to_string(),
                format!("object(layout={layout},slot{tag_slot}=1,slot{payload_slot}=null)"),
                format!("object(layout={layout},slot{tag_slot}=1,slot{payload_slot}=7)"),
            ],
        }
    }

    #[test]
    fn managed_utf8_parameter_uses_frozen_text_layout() {
        let ty = nyar("utf8");
        let mut plan = plan("greet", function(&[(0, "name", ty.clone())], nyar("unit")));
        plan.testing_register_type_layout(
            ty,
            layout(11, "Utf8Text", StorageKind::Reference, vec![("_repr", NyarType::Array(Box::new(nyar("u8"))))]),
        );
        let contract = derive(&plan).expect("托管文本必须有 OVM 合同");
        let export = export_of(&contract);
        assert_eq!(
            export.params[0].ty,
            OvmAbiType::Text { representation: OvmTextRepresentation::Managed { layout: 0, field_count: 1, byte_slot: 0 } }
        );
        assert_eq!(export.params[0].carrier, "handle");
    }

    #[test]
    fn text_layout_without_single_byte_array_slot_fails_closed() {
        let ty = nyar("utf8");
        let mut plan = plan("greet", function(&[(0, "name", ty.clone())], nyar("unit")));
        plan.testing_register_type_layout(ty, layout(11, "Utf8Text", StorageKind::Reference, vec![("_repr", nyar("i64"))]));
        let error = derive(&plan).expect_err("非 `[u8]` 文本布局必须失败");
        assert!(error.contains("OVM 文本载体"), "got: {error}");
    }

    #[test]
    fn same_layout_facts_derive_same_digest() {
        let ty = NyarType::Nominal { layout: 7, spelling: Identifier::new("demo::Point") };
        let build = || {
            let mut plan = plan("point", function(&[(0, "point", ty.clone())], ty.clone()));
            plan.testing_register_type_layout(ty.clone(), layout(7, "Point", StorageKind::Value, vec![("x", nyar("i64")), ("y", nyar("i64"))]));
            derive(&plan).expect("合同")
        };
        let first = build();
        let second = build();
        assert_eq!(first.digest, second.digest, "同一布局事实必须派生同一 digest");
        assert_eq!(first.exports, second.exports);
    }

    #[test]
    fn unknown_layout_fails_closed() {
        let ty = NyarType::Nominal { layout: 99, spelling: Identifier::new("demo::Ghost") };
        let plan = plan("ghost", function(&[(0, "ghost", ty.clone())], ty));
        let error = derive(&plan).expect_err("未注册布局必须失败");
        assert!(error.contains("未注册"), "got: {error}");
    }

    #[test]
    fn unsupported_scalar_export_fails_before_contract() {
        for ty in [
            nyar("i128"),
            NyarType::Utf16,
            NyarType::TraitObject(nyar_types::WitnessObject { trait_path: Identifier::new("demo::Shape"), type_arguments: Vec::new() }),
        ] {
            let plan = plan_with("bad", function(&[(0, "value", nyar("i64"))], ty));
            let error = derive(&plan).expect_err("不支持的导出类型必须失败");
            assert!(!error.is_empty(), "错误必须携带具体原因");
        }
    }

    #[test]
    fn empty_boundary_exports_get_a_contract_with_stable_digest() {
        // OVM 的导出面来自 `exported_operations` / 程序入口，不在 wasm 冻结缓存里。
        // 零参数、`Void` 返回或完全无可携带内容都必须拿到**空的但存在**的合同，
        // 不能把“没有需要携带的东西”误判成“缺少载体合同”。
        let cases: [(&str, Vec<(usize, &str, NyarType)>, NyarType, usize, &str); 3] = [
            ("zero_void", Vec::new(), nyar("unit"), 0, "void"),
            ("zero_i64", Vec::new(), nyar("i64"), 0, "i64"),
            ("arg_void", vec![(0, "value", nyar("i64"))], nyar("unit"), 1, "void"),
        ];
        for (name, params, returns, param_count, returns_carrier) in cases {
            let build = || {
                let plan = plan_with(name, function(&params, returns.clone()));
                derive_with_layouts(&plan).expect("空边界必须有合同")
            };
            let (contract, layouts) = build();
            let export = export_of(&contract);
            assert!(layouts.is_empty(), "空边界不得凭空登记布局: {layouts:?}");
            assert_eq!(export.params.len(), param_count, "{name} 的参数槽数必须与声明一致");
            assert_eq!(export.returns_carrier, returns_carrier, "{name} 的返回载体");
            assert!(contract.digest.starts_with("sha256-"), "{name} digest: {}", contract.digest);
            let (again, _) = build();
            assert_eq!(contract.digest, again.digest, "{name} 相同边界事实必须派生稳定 digest");
        }
    }

    /// 不可携带的边界类型必须由本 lane 的结构投影直接失败，不依赖任何外部
    /// 平台的预冻结载体表。
    #[test]
    fn unsupported_export_types_fail_derive() {
        for ty in [
            nyar("i128"),
            NyarType::Utf16,
            NyarType::TraitObject(nyar_types::WitnessObject { trait_path: Identifier::new("demo::Shape"), type_arguments: Vec::new() }),
        ] {
            let plan = plan_with("bad", function(&[(0, "value", nyar("i64"))], ty));
            let error = derive(&plan).expect_err("不可携带类型必须结构化失败");
            assert!(error.contains("没有 OVM ABI 合同"), "got: {error}");
        }
    }

    #[test]
    fn unsupported_aggregate_field_fails_closed() {
        let ty = NyarType::Nominal { layout: 8, spelling: Identifier::new("demo::Heavy") };
        let mut plan = plan("heavy", function(&[(0, "heavy", ty.clone())], ty.clone()));
        plan.testing_register_type_layout(ty, layout(8, "Heavy", StorageKind::Value, vec![("big", nyar("i128")), ("name", nyar("utf8"))]));
        let error = derive(&plan).expect_err("聚合字段不支持必须失败");
        assert!(error.contains("Heavy"), "got: {error}");
        assert!(error.contains("没有 OVM ABI 合同"), "got: {error}");
    }

    #[test]
    fn digest_changes_with_layout_shape() {
        let ty = NyarType::Nominal { layout: 7, spelling: Identifier::new("demo::Point") };
        let build = |fields: Vec<(&str, NyarType)>| {
            let mut plan = plan("point", function(&[(0, "point", ty.clone())], ty.clone()));
            plan.testing_register_type_layout(ty.clone(), layout(7, "Point", StorageKind::Value, fields));
            derive(&plan).expect("合同").digest
        };
        let narrow = build(vec![("x", nyar("i64"))]);
        let wide = build(vec![("x", nyar("i64")), ("y", nyar("i64"))]);
        assert_ne!(narrow, wide, "布局字段形状必须进入 digest");
    }

    #[test]
    fn unite_projects_variant_payloads_with_frozen_slots() {
        let sum_ty = NyarType::Nominal { layout: 42, spelling: Identifier::new("demo::Maybe") };
        let mut plan = plan("maybe", function(&[(0, "value", sum_ty.clone())], sum_ty.clone()));
        plan.testing_register_sum_layout(
            sum_ty,
            nyar_types::SumTypeLayout {
                nominal: nyar_types::NominalInstanceId::from_index(0).expect("nominal"),
                name: "Maybe".to_string(),
                is_unite: true,
                variants: vec![
                    nyar_types::SumVariantLayout {
                        id: nyar_types::VariantId::from_index(0).expect("variant"),
                        name: "None".to_string(),
                        tag: 0,
                        payload_type: None,
                    },
                    nyar_types::SumVariantLayout {
                        id: nyar_types::VariantId::from_index(1).expect("variant"),
                        name: "Some".to_string(),
                        tag: 1,
                        payload_type: Some(nyar("i64")),
                    },
                ],
            },
        );
        let (contract, layouts) = derive_with_layouts(&plan).expect("unite 必须派生 OVM 合同");
        let OvmAbiType::Unite { layout, name, tag_slot, payload_slot, variants } = &export_of(&contract).params[0].ty
        else {
            panic!("unite 必须投影为 Unite 合同")
        };
        assert_eq!(*layout, 0, "unite 布局下标来自 lowering 登记表");
        assert_eq!(layouts.len(), 1);
        assert_eq!(layouts[0].field_count, 2, "unite 物理布局是 [tag, payload]");
        assert_eq!(name, "Maybe");
        assert_eq!((*tag_slot, *payload_slot), (0, 1), "tag / payload 槽位是冻结配方");
        assert_eq!(
            variants.iter().map(|variant| (variant.name.as_str(), variant.tag, variant.payload.clone())).collect::<Vec<_>>(),
            vec![("None", 0, None), ("Some", 1, Some(Box::new(OvmAbiType::I64)))],
            "带载荷变体必须携带自己的 payload 结构，零载荷变体不得伪造载荷"
        );
    }
}
