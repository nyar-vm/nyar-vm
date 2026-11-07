//! OVM target 的库导出 ABI 合同。
//!
//! 合同全部由 [`TargetExecutionPlan`] 冻结的类型、载体和布局事实派生：结构层与
//! wasm 共用 [`crate::abi_structure`]，本模块只负责 OVM 物理投影（模块布局下标与
//! 无类型槽位），不复制结构推导。
//!
//! 失败闭合是设计的一部分：
//! - 未注册布局、`i128`、`Utf16`、`TraitObject`、未知/不支持结构一律返回错误；
//! - 任何 `Unsupported` 都不会进入 JSON，也不会被降级成“先跑再说”的兜底；
//! - 参数/返回的物理载体必须与 plan 冻结的 [`LibraryExportAbi`] 一致，不一致即失败。
//!
//! digest 覆盖规范化 JSON 里的 `schema_version` 与全部导出（按逻辑名排序），
//! 同一布局事实在任意导出位置派生同一 digest。

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{AggregateLayoutPlan, ItemInstanceId, LayoutId, NominalInstanceId, NyarType, combined_content_hash};
use odin_bytecode::OdinLayout;
use serde::{Deserialize, Serialize};

use crate::abi_structure::{
    AbiStructure, AggregateStructure, SequenceStructure, abi_structure_for_type_with_layouts, canonical_parameter_identities,
};
use crate::backend_plan_views::ExecutableFunction;
use crate::{AbiCarrier, LibraryExportAbi, Result, TargetExecutionPlan};

/// OVM 库导出 ABI schema 版本（合同命名空间 `/v1`）。
pub const OVM_ABI_SCHEMA_VERSION: u32 = 1;

/// OVM 库导出 ABI 合同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiContract {
    /// schema 版本。
    pub schema_version: u32,
    /// 目标车道标识（固定 `ovm`）。
    pub lane: String,
    /// 按逻辑名排序的导出合同。
    pub exports: Vec<OvmAbiExport>,
    /// 覆盖 `schema_version` 与全部导出的稳定 digest（`sha256-<hex>`）。
    pub digest: String,
}

/// 单个库导出的 OVM ABI 合同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiExport {
    /// 逻辑导出名。
    pub logical_name: String,
    /// 按声明顺序排列的参数。
    pub params: Vec<OvmAbiParam>,
    /// 返回值结构；无返回值时为 [`OvmAbiType::Unit`]。
    pub returns: OvmAbiType,
    /// 返回值物理载体（来自 plan 冻结合同）。
    pub returns_carrier: String,
}

/// 单个入口参数的 OVM ABI 合同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiParam {
    /// 声明下标（0 起连续）。
    pub index: usize,
    /// canonical 参数名。
    pub name: String,
    /// 参数结构。
    #[serde(rename = "type")]
    pub ty: OvmAbiType,
    /// 物理载体（来自 plan 冻结合同）。
    pub carrier: String,
}

/// OVM 物理投影后的 ABI 类型。
///
/// 标量保留语义身份，宿主按该身份编解码；聚合与序列携带模块布局下标与槽位，
/// 宿主据此在 OVM 堆上装配同一形状的对象。所有槽位都是声明顺序事实。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum OvmAbiType {
    /// 无值（仅返回位）。
    Unit,
    /// 32-bit 整数。
    I32,
    /// 64-bit 整数。
    I64,
    /// 32-bit 浮点。
    F32,
    /// 64-bit 浮点。
    F64,
    /// 布尔。
    Bool,
    /// Unicode 标量字符。
    Char,
    /// 文本值（`utf8`）。文本不是标量，物理形态由冻结文本边界决定。
    Text {
        /// 文本边界表示。
        representation: OvmTextRepresentation,
    },
    /// 堆数组 `[T]`；宿主按元素结构装配等长槽位对象。
    Array {
        /// 元素结构。
        element: Box<OvmAbiType>,
    },
    /// 定长数组 `[T; N]`。
    FixedArray {
        /// 元素结构。
        element: Box<OvmAbiType>,
        /// 元素数量。
        length: usize,
    },
    /// `T?`：`null` 表示 `None`，其余槽位值即载荷。
    Option {
        /// 非空载荷结构。
        inner: Box<OvmAbiType>,
    },
    /// 位置有语义的产品聚合。
    Tuple {
        /// 按声明顺序排列的字段。
        fields: Vec<OvmAbiType>,
    },
    /// 值存储具名聚合：宿主按 `layout` + 槽位装配。
    Aggregate {
        /// 模块布局下标（`.odin` `layouts` 表下标）。
        layout: i32,
        /// 简单名。
        name: String,
        /// 完整命名空间路径。
        module_path: String,
        /// 按声明顺序排列的字段；字段在对象里的槽位等于其下标。
        fields: Vec<OvmAbiField>,
    },
    /// 序列容器视图（冻结布局派生的 `items + length` 形状）。
    Sequence {
        /// 容器模块布局下标。
        layout: i32,
        /// 容器简单名。
        name: String,
        /// 容器完整命名空间路径。
        module_path: String,
        /// backing array 字段槽位。
        items_slot: usize,
        /// 计数字段槽位。
        length_slot: usize,
        /// 容器字段总数。
        field_count: usize,
        /// backing array 元素结构。
        element: Box<OvmAbiType>,
        /// 计数字段结构。
        length_type: Box<OvmAbiType>,
    },
    /// 引用存储名义实例的共享身份（不是字段快照）。
    InstanceHandle {
        /// 模块布局下标。
        layout: i32,
        /// 简单名。
        name: String,
        /// 完整命名空间路径。
        module_path: String,
    },
    /// 零 payload unite：物理布局由模块 sum 布局下标定位；tag 与 payload 槽位
    /// 都是 OVM lowering 已冻结的模块布局事实。
    Unite {
        /// 模块 sum 布局下标（`.odin` `layouts` 表下标）。
        layout: i32,
        /// sum 简单名。
        name: String,
        /// 按声明顺序排列的变体。
        variants: Vec<OvmAbiVariant>,
    },
}

/// 具名聚合字段合同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiField {
    /// 字段名（来自冻结布局）。
    pub name: String,
    /// 字段结构。
    #[serde(rename = "type")]
    pub ty: OvmAbiType,
}

/// `utf8` 文本值的 OVM 边界表示。
///
/// 判据：文本不是标量。`utf8` 的物理形态由目标计划为**该槽**冻结的文本布局
/// 决定，不由类型名或目标名猜测：
/// - 冻结了文本布局 → 托管文本对象（Reference 存储，单 `[u8]` 槽，UTF-8 字节）；
/// - 没冻结 → raw 宿主串（`Value::String` / JSON string）。
///
/// wasm 不存在这两个分支：JS glue 用 intern 表把字符串映射为 `i32` 句柄，
/// 因此 plan 的 `I32` 只是 wasm 物理事实，OVM 不得复用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum OvmTextRepresentation {
    /// 直接过界：JSON string ↔ `Value::String`。
    Raw,
    /// 托管文本对象：`layout` 的 `byte_slot` 槽是 UTF-8 字节数组。
    Managed {
        /// 文本对象在 `.odin` 布局表里的下标。
        layout: i32,
        /// 文本对象字段总数（冻结配方恒为 1）。
        field_count: usize,
        /// UTF-8 字节数组所在槽位（冻结配方恒为 0）。
        byte_slot: usize,
    },
}

/// unite 变体身份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiVariant {
    /// 变体名。
    pub name: String,
    /// 判别 tag。
    pub tag: u32,
}

impl OvmAbiContract {
    /// 从冻结计划派生合同。
    ///
    /// `exports` 是 `(实例身份, 逻辑导出名)`；`module_layouts` / `layout_indices`
    /// 是 `.odin` 模块已登记的聚合布局表与 canonical `LayoutId` → 模块下标映射；
    /// `sum_layout_indices` 是 lowering 已登记的 `NominalInstanceId` → sum 布局下标。
    /// ABI 位置引用但模块尚未登记的布局会按冻结字段数追加到表尾，保证合同里
    /// 的每个聚合下标都对应一份真实布局。
    pub fn derive(
        plan: &TargetExecutionPlan,
        exports: &[(ItemInstanceId, String)],
        module_layouts: &mut Vec<OdinLayout>,
        layout_indices: &mut BTreeMap<LayoutId, i32>,
        sum_layout_indices: &BTreeMap<NominalInstanceId, i32>,
    ) -> Result<Self> {
        let mut entries = Vec::with_capacity(exports.len());
        let mut seen = BTreeSet::new();
        for (instance, logical_name) in exports {
            if !seen.insert(logical_name.clone()) {
                return Err(format!("库导出名 `{logical_name}` 重复，无法派生 OVM ABI 合同"));
            }
            let view = plan
                .get_function(instance)
                .ok_or_else(|| format!("库导出 `{logical_name}` 缺少 Compiler 函数体，无法派生 OVM ABI 合同"))?;
            let abi = plan
                .library_export_abi(instance)
                .ok_or_else(|| format!("库导出 `{logical_name}` 缺少冻结 ABI 载体合同，无法派生 OVM ABI 合同"))?;
            entries.push(derive_export(
                plan,
                logical_name,
                &view.function,
                abi,
                module_layouts,
                layout_indices,
                sum_layout_indices,
            )?);
        }
        // 逻辑名排序让 digest 与导出登记顺序无关。
        entries.sort_by(|left, right| left.logical_name.cmp(&right.logical_name));
        let digest = contract_digest(&entries)?;
        Ok(Self { schema_version: OVM_ABI_SCHEMA_VERSION, lane: "ovm".to_string(), exports: entries, digest })
    }

    /// 按逻辑名查询导出合同。
    pub fn export(&self, logical_name: &str) -> Option<&OvmAbiExport> {
        self.exports.iter().find(|export| export.logical_name == logical_name)
    }
}

fn derive_export(
    plan: &TargetExecutionPlan,
    logical_name: &str,
    function: &ExecutableFunction,
    abi: &LibraryExportAbi,
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
    sum_layout_indices: &BTreeMap<NominalInstanceId, i32>,
) -> Result<OvmAbiExport> {
    let identities = canonical_parameter_identities(logical_name, function)?;
    if abi.params.len() != identities.len() {
        return Err(format!(
            "库导出 `{logical_name}` 冻结载体数（{}）与参数身份数（{}）不一致",
            abi.params.len(),
            identities.len()
        ));
    }
    let mut params = Vec::with_capacity(identities.len());
    for (position, (_, name)) in identities.iter().enumerate() {
        let ty = function
            .param_types
            .get(position)
            .ok_or_else(|| format!("库导出 `{logical_name}` 参数 {position} 缺少类型事实"))?;
        let structure =
            abi_structure_for_type_with_layouts(ty, plan.aggregate_layouts(), plan.aggregate_layout_by_nyar_type(), plan.sum_layout_by_nyar_type());
        let projected = project(
            &structure,
            plan,
            module_layouts,
            layout_indices,
            sum_layout_indices,
            parameter_text_layout(plan, function, position),
        )
        .map_err(|reason| format!("库导出 `{logical_name}` 参数 `{name}` 没有 OVM ABI 合同: {reason}"))?;
        verify_carrier(abi.params[position], &projected)
            .map_err(|reason| format!("库导出 `{logical_name}` 参数 `{name}` 载体合同不一致: {reason}"))?;
        params.push(OvmAbiParam {
            index: position,
            name: name.to_string(),
            carrier: carrier_label(&projected).to_string(),
            ty: projected,
        });
    }
    let return_structure = abi_structure_for_type_with_layouts(
        &function.return_type,
        plan.aggregate_layouts(),
        plan.aggregate_layout_by_nyar_type(),
        plan.sum_layout_by_nyar_type(),
    );
    let returns = project(
        &return_structure,
        plan,
        module_layouts,
        layout_indices,
        sum_layout_indices,
        return_text_layout(plan, function),
    )
    .map_err(|reason| format!("库导出 `{logical_name}` 返回类型没有 OVM ABI 合同: {reason}"))?;
    verify_carrier(abi.returns, &returns)
        .map_err(|reason| format!("库导出 `{logical_name}` 返回载体合同不一致: {reason}"))?;
    Ok(OvmAbiExport {
        logical_name: logical_name.to_string(),
        params,
        returns_carrier: carrier_label(&returns).to_string(),
        returns,
    })
}

/// 参数槽的冻结文本布局身份：与 OVM lowering 的入口形参表同源
/// （`value_layouts` 优先，其次按折叠后类型回查 `aggregate_layout_by_nyar_type`）。
fn parameter_text_layout(plan: &TargetExecutionPlan, function: &ExecutableFunction, position: usize) -> Option<LayoutId> {
    let parameter = function
        .blocks
        .get(function.entry.0 as usize)
        .and_then(|block| block.parameters.get(position))
        .copied()?;
    function
        .value_layouts
        .get(&parameter)
        .copied()
        .or_else(|| plan.aggregate_layout_by_nyar_type().get(&function.param_types[position]).copied())
}

/// 返回槽的冻结文本布局身份：`return_layout` 优先，其次按折叠后类型回查。
fn return_text_layout(plan: &TargetExecutionPlan, function: &ExecutableFunction) -> Option<LayoutId> {
    function
        .return_layout
        .or_else(|| plan.aggregate_layout_by_nyar_type().get(&function.return_type).copied())
}

/// 结构 → OVM 物理投影。
///
/// `Unsupported` 与未注册布局都在这里结构化失败；投影成功的结果不含任何
/// “未知”分支，因此宿主不需要也不允许再猜。
fn project(
    structure: &AbiStructure,
    plan: &TargetExecutionPlan,
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
    sum_layout_indices: &BTreeMap<NominalInstanceId, i32>,
    text_layout: Option<LayoutId>,
) -> Result<OvmAbiType> {
    Ok(match structure {
        AbiStructure::Unit => OvmAbiType::Unit,
        AbiStructure::I32 => OvmAbiType::I32,
        AbiStructure::I64 => OvmAbiType::I64,
        AbiStructure::F32 => OvmAbiType::F32,
        AbiStructure::F64 => OvmAbiType::F64,
        AbiStructure::Bool => OvmAbiType::Bool,
        AbiStructure::Char => OvmAbiType::Char,
        AbiStructure::Utf8 => project_text(plan, module_layouts, layout_indices, text_layout)?,
        AbiStructure::Array { element, .. } => OvmAbiType::Array {
            element: Box::new(project(element, plan, module_layouts, layout_indices, sum_layout_indices, None)?),
        },
        AbiStructure::FixedArray { element, length } => OvmAbiType::FixedArray {
            element: Box::new(project(element, plan, module_layouts, layout_indices, sum_layout_indices, None)?),
            length: *length,
        },
        AbiStructure::Option { inner } => OvmAbiType::Option {
            inner: Box::new(project(inner, plan, module_layouts, layout_indices, sum_layout_indices, None)?),
        },
        AbiStructure::Tuple { fields } => OvmAbiType::Tuple {
            fields: fields
                .iter()
                .map(|field| project(field, plan, module_layouts, layout_indices, sum_layout_indices, None))
                .collect::<Result<Vec<_>>>()?,
        },
        AbiStructure::Aggregate(aggregate) => {
            project_aggregate(aggregate, plan, module_layouts, layout_indices, sum_layout_indices)?
        }
        AbiStructure::Sequence(sequence) => {
            project_sequence(sequence, plan, module_layouts, layout_indices, sum_layout_indices)?
        }
        AbiStructure::InstanceHandle { layout, name, module_path } => {
            let aggregate = plan
                .aggregate_layouts()
                .layouts
                .iter()
                .find(|item| item.id == *layout)
                .ok_or_else(|| format!("layout `{layout}` 未注册，无法投影引用实例 `{module_path}::{name}`"))?;
            OvmAbiType::InstanceHandle {
                layout: layout_index(*layout, aggregate.fields.len(), module_layouts, layout_indices)?,
                name: name.clone(),
                module_path: module_path.clone(),
            }
        }
        AbiStructure::Unite { nominal, name, variants } => {
            // 物理 sum 布局由 lowering 已登记的 nominal → 模块下标映射给出；
            // 缺失即失败闭合，绝不在合同里留“未知布局”。
            let layout = *sum_layout_indices.get(nominal).ok_or_else(|| {
                format!("sum nominal `{nominal:?}` 没有已登记的 OVM 模块布局，拒绝按未知布局解释内存")
            })?;
            OvmAbiType::Unite {
                layout,
                name: name.clone(),
                variants: variants.iter().map(|variant| OvmAbiVariant { name: variant.name.clone(), tag: variant.tag }).collect(),
            }
        }
        AbiStructure::Unsupported { reason } => return Err(reason.clone()),
    })
}

/// `utf8` 文本边界投影。
///
/// 该槽冻结了文本布局 → 托管文本对象（必须通过冻结的单一 `[u8]` 槽配方校验）；
/// 没有冻结 → raw 宿主串。绝不把 plan 的 wasm `I32` 句柄当 OVM 标量。
fn project_text(
    plan: &TargetExecutionPlan,
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
    slot_layout: Option<LayoutId>,
) -> Result<OvmAbiType> {
    let layout_id = slot_layout.or_else(|| plan.aggregate_layout_by_nyar_type().get(&NyarType::Utf8).copied());
    let Some(layout_id) = layout_id
    else {
        return Ok(OvmAbiType::Text { representation: OvmTextRepresentation::Raw });
    };
    let layout = find_layout(plan.aggregate_layouts(), layout_id)?;
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
            layout: layout_index(layout_id, layout.fields.len(), module_layouts, layout_indices)?,
            field_count: layout.fields.len(),
            byte_slot: 0,
        },
    })
}

fn project_aggregate(
    aggregate: &AggregateStructure,
    plan: &TargetExecutionPlan,
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
    sum_layout_indices: &BTreeMap<NominalInstanceId, i32>,
) -> Result<OvmAbiType> {
    let layout_id = aggregate
        .layout
        .ok_or_else(|| format!("聚合 `{}::{}` 没有冻结布局身份，OVM 拒绝按身份猜字段", aggregate.module_path, aggregate.name))?;
    let layout = find_layout(plan.aggregate_layouts(), layout_id)?;
    if layout.storage.is_reference() {
        return Err(format!(
            "聚合 `{}::{}` 是引用存储，必须投影为共享身份而不是字段快照",
            layout.namespace, layout.name
        ));
    }
    let mut fields = Vec::with_capacity(aggregate.fields.len());
    for field in &aggregate.fields {
        fields.push(OvmAbiField {
            name: field.name.clone(),
            ty: project(&field.ty, plan, module_layouts, layout_indices, sum_layout_indices, None)
                .map_err(|reason| format!("聚合 `{}::{}` 字段 `{}` 没有 OVM ABI 合同: {reason}", layout.namespace, layout.name, field.name))?,
        });
    }
    if fields.len() != layout.fields.len() {
        return Err(format!(
            "聚合 `{}::{}` 的合同字段数（{}）与冻结布局字段数（{}）不一致",
            layout.namespace,
            layout.name,
            fields.len(),
            layout.fields.len()
        ));
    }
    Ok(OvmAbiType::Aggregate {
        layout: layout_index(layout_id, layout.fields.len(), module_layouts, layout_indices)?,
        name: aggregate.name.clone(),
        module_path: aggregate.module_path.clone(),
        fields,
    })
}

fn project_sequence(
    sequence: &SequenceStructure,
    plan: &TargetExecutionPlan,
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
    sum_layout_indices: &BTreeMap<NominalInstanceId, i32>,
) -> Result<OvmAbiType> {
    let layout = find_layout(plan.aggregate_layouts(), sequence.layout)?;
    if !layout.storage.is_reference() {
        return Err(format!(
            "序列容器 `{}::{}` 的冻结布局不是引用存储，拒绝派生序列投影",
            layout.namespace, layout.name
        ));
    }
    if layout.fields.len() != sequence.field_count {
        return Err(format!(
            "序列容器 `{}::{}` 的合同字段数（{}）与冻结布局字段数（{}）不一致",
            layout.namespace,
            layout.name,
            sequence.field_count,
            layout.fields.len()
        ));
    }
    Ok(OvmAbiType::Sequence {
        layout: layout_index(sequence.layout, layout.fields.len(), module_layouts, layout_indices)?,
        name: sequence.name.clone(),
        module_path: sequence.module_path.clone(),
        items_slot: sequence.items_slot,
        length_slot: sequence.length_slot,
        field_count: sequence.field_count,
        element: Box::new(project(&sequence.element, plan, module_layouts, layout_indices, sum_layout_indices, None)?),
        length_type: Box::new(project(&sequence.length_ty, plan, module_layouts, layout_indices, sum_layout_indices, None)?),
    })
}

fn find_layout(plan: &AggregateLayoutPlan, layout_id: LayoutId) -> Result<&nyar_types::AggregateLayout> {
    plan.layouts
        .iter()
        .find(|item| item.id == layout_id)
        .ok_or_else(|| format!("layout `{layout_id}` 未注册，无法派生 OVM 物理投影"))
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

/// 交叉核对 plan 冻结载体与结构投影；不一致即失败闭合。
///
/// `utf8` 特例：plan 载体是 wasm 物理事实（JS glue 的 intern `i32` 句柄），
/// 不是 OVM 文本边界。OVM 文本载体由 [`OvmTextRepresentation`] 声明，因此这里
/// 只校验 plan 确实把它冻结成 `i32`，不与 OVM 的 `text` / `handle` 标签比较。
fn verify_carrier(expected: AbiCarrier, ty: &OvmAbiType) -> Result<()> {
    if matches!(ty, OvmAbiType::Text { .. }) {
        if expected != AbiCarrier::I32 {
            return Err(format!("冻结载体 `{}` 不是 `utf8` 的 wasm 物理载体 `i32`", expected.as_str()));
        }
        return Ok(());
    }
    let actual = carrier_of(ty);
    if expected != actual {
        return Err(format!("冻结载体 `{}` 与结构投影载体 `{}` 不一致", expected.as_str(), actual.as_str()));
    }
    Ok(())
}

/// 载体的 JSON 标签。
///
/// `utf8` 不使用 plan 的 wasm `i32`：raw 宿主串标 `text`，托管文本对象标
/// `handle`，消费端据此选择 JSON ↔ 值的物化路径。
fn carrier_label(ty: &OvmAbiType) -> &'static str {
    match ty {
        OvmAbiType::Text { representation: OvmTextRepresentation::Raw } => "text",
        OvmAbiType::Text { representation: OvmTextRepresentation::Managed { .. } } => "handle",
        other => carrier_of(other).as_str(),
    }
}

fn carrier_of(ty: &OvmAbiType) -> AbiCarrier {
    match ty {
        OvmAbiType::Unit => AbiCarrier::Void,
        // `Text` 的 plan 载体是 wasm intern `i32`；OVM 标签由 `carrier_label` 给出。
        OvmAbiType::I32 | OvmAbiType::Bool | OvmAbiType::Char | OvmAbiType::Text { .. } => AbiCarrier::I32,
        OvmAbiType::I64 => AbiCarrier::I64,
        OvmAbiType::F32 => AbiCarrier::F32,
        OvmAbiType::F64 => AbiCarrier::F64,
        OvmAbiType::Array { .. }
        | OvmAbiType::FixedArray { .. }
        | OvmAbiType::Option { .. }
        | OvmAbiType::Tuple { .. }
        | OvmAbiType::Aggregate { .. }
        | OvmAbiType::Sequence { .. }
        | OvmAbiType::InstanceHandle { .. }
        | OvmAbiType::Unite { .. } => AbiCarrier::Handle,
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
        NyarType::from_surface_primitive(s).expect("surface primitive")
    }

    #[allow(deprecated)]
    fn function(parameters: &[(usize, &str, NyarType)], return_type: NyarType) -> ExecutableFunction {
        ExecutableFunction {
            symbol: "exported".to_string(),
            return_type,
            param_types: parameters.iter().map(|(_, _, ty)| ty.clone()).collect(),
            value_types: BTreeMap::new(),
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
                parameters: Vec::new(),
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
            plan.testing_wasm_export_names_mut().insert(instance, export.to_string());
        }
        plan
    }

    fn plan(export: &str, function: ExecutableFunction) -> TargetExecutionPlan {
        let mut plan = plan_with(export, function);
        plan.testing_freeze_library_export_abis().expect("载体冻结");
        plan
    }

    fn derive(plan: &TargetExecutionPlan) -> Result<OvmAbiContract> {
        let exports = plan.instances().into_iter().map(|instance| (instance, "exported".to_string())).collect::<Vec<_>>();
        let mut module_layouts = Vec::new();
        let mut layout_indices = BTreeMap::new();
        let sum_layout_indices = BTreeMap::new();
        OvmAbiContract::derive(plan, &exports, &mut module_layouts, &mut layout_indices, &sum_layout_indices)
    }

    fn layout(id: LayoutId, name: &str, storage: StorageKind, fields: Vec<(&str, NyarType)>) -> AggregateLayout {
        AggregateLayout {
            id,
            name: name.to_string(),
            namespace: "demo".to_string(),
            key: None,
            storage,
            size: 0,
            align: 1,
            fields: fields
                .into_iter()
                .map(|(name, ty)| FieldLayout { name: name.to_string(), ty, offset: 0, size: 0, align: 1 })
                .collect(),
        }
    }

    fn export_of(contract: &OvmAbiContract) -> &OvmAbiExport {
        contract.exports.first().expect("至少一个导出")
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
        plan.testing_register_type_layout(
            ty,
            layout(7, "Point", StorageKind::Value, vec![("x", nyar("i64")), ("y", nyar("i64"))]),
        );
        let contract = derive(&plan).expect("具名聚合必须有 OVM 合同");
        let OvmAbiType::Aggregate { layout: index, name, module_path, fields } = &export_of(&contract).params[0].ty else {
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
        plan.testing_register_type_layout(inner_ty.clone(), layout(8, "Inner", StorageKind::Value, vec![("value", nyar("i64")), ("tag", nyar("i32"))]));
        plan.testing_register_type_layout(
            outer_ty,
            layout(7, "Outer", StorageKind::Value, vec![("inner", inner_ty), ("count", nyar("i32"))]),
        );
        let contract = derive(&plan).expect("嵌套聚合必须有 OVM 合同");
        let OvmAbiType::Aggregate { fields, .. } = &export_of(&contract).params[0].ty else {
            panic!("外层必须是 Aggregate")
        };
        let OvmAbiType::Aggregate { layout: inner_index, fields: inner_fields, .. } = &fields[0].ty else {
            panic!("嵌套字段必须是 Aggregate")
        };
        assert_eq!(*inner_index, 0);
        assert_eq!(inner_fields.len(), 2);
        assert_eq!(fields[1].ty, OvmAbiType::I32);
    }

    #[test]
    fn generic_sequence_instance_projects_items_and_length_slots() {
        let list = NyarType::Apply(Box::new(NyarType::Named(Identifier::new("std::collections::ArrayList"))), vec![nyar("i64")]);
        let mut plan = plan("list", function(&[(0, "values", list.clone())], list.clone()));
        plan.testing_register_type_layout(
            list,
            layout(
                9,
                "ArrayList",
                StorageKind::Reference,
                vec![("_items", NyarType::Array(Box::new(nyar("i64")))), ("_length", nyar("i32"))],
            ),
        );
        let contract = derive(&plan).expect("泛型实例必须有 OVM 合同");
        let OvmAbiType::Sequence { layout: index, items_slot, length_slot, field_count, element, length_type, .. } =
            &export_of(&contract).params[0].ty
        else {
            panic!("序列形状必须投影为 Sequence")
        };
        assert_eq!(*index, 0);
        assert_eq!((*items_slot, *length_slot, *field_count), (0, 1, 2));
        assert_eq!(*element.as_ref(), OvmAbiType::I64);
        assert_eq!(*length_type.as_ref(), OvmAbiType::I32);
    }

    #[test]
    fn raw_utf8_parameter_uses_host_text_carrier() {
        let ty = nyar("utf8");
        let plan = plan("greet", function(&[(0, "name", ty.clone())], ty));
        let contract = derive(&plan).expect("`utf8` 必须有 OVM 合同");
        let export = export_of(&contract);
        assert_eq!(export.params[0].ty, OvmAbiType::Text { representation: OvmTextRepresentation::Raw });
        assert_eq!(export.params[0].carrier, "text");
        assert_eq!(export.returns, OvmAbiType::Text { representation: OvmTextRepresentation::Raw });
        assert_eq!(export.returns_carrier, "text");
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
            plan.testing_register_type_layout(
                ty.clone(),
                layout(7, "Point", StorageKind::Value, vec![("x", nyar("i64")), ("y", nyar("i64"))]),
            );
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
            let mut plan = plan_with("bad", function(&[(0, "value", nyar("i64"))], ty));
            let error = plan.testing_freeze_library_export_abis().expect_err("不支持的导出类型必须失败");
            assert!(error.contains("冻结 ABI 载体合同失败"), "got: {error}");
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
}
