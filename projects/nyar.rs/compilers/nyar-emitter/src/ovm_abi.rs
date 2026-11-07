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
use sha2::{Digest, Sha256};

use crate::abi_structure::{
    AbiStructure, AggregateStructure, SequenceStructure, abi_structure_for_type_with_layouts, canonical_parameter_identities,
};
use crate::backend_plan_views::ExecutableFunction;
use crate::lowering::shared::executable::slots::ExecutableSlotPlan;
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
    /// `T?`：物理表示由 [`OvmOptionBoxing`] 显式声明，宿主不得按“空对象”推断。
    Option {
        /// 非空载荷结构。
        inner: Box<OvmAbiType>,
        /// 显式 box 语义：嵌套 Option 必须带判别 tag，禁止用 `null` 同时表示
        /// 外层 `None` 与内层 `None`。
        boxing: OvmOptionBoxing,
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

/// `Option` 的 OVM 物理 box 语义。
///
/// `Some(None)` 与外层 `None` 在“用 `null` 表示 None”的表示下物理同形——这是
/// 类型信息在边界上的真实丢失，不是序列化细节。因此合同必须显式声明：
///
/// - [`Niche`](Self::Niche)：内层不是 `Option`。`None` = `Value::Null`，
///   `Some(x)` = `x` 本身。
/// - [`Boxed`](Self::Boxed)：内层是 `Option`。三种形态的物理表示分别是
///   `None` = `Value::Null`、
///   `Some(None)` = 2 槽对象 `tag=1, payload=Value::Null`、
///   `Some(Some(x))` = 2 槽对象 `tag=1, payload=x`（`x` 再按内层合同解释）。
///
/// 判别 tag 的冻结配方向宿主暴露为 `tag_slot` / `payload_slot`：`0` 是 `None`，
/// `1` 是 `Some`。消费端只按合同取，禁止“是 null 就当 None”或“空对象当 None”
/// 的推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum OvmOptionBoxing {
    /// 内层物理表示不含 `null` 歧义时使用。
    Niche,
    /// 内层本身是 `Option` 时强制显式 box。
    Boxed {
        /// box 对象在 `.odin` 布局表里的下标（冻结配方恒为 2 槽）。
        layout: i32,
        /// 判别 tag 槽位（冻结配方恒为 0；`0 = None`、`1 = Some`）。
        tag_slot: usize,
        /// 载荷槽位（冻结配方恒为 1）。
        payload_slot: usize,
    },
}

/// OVM 库导出 sidecar 的固定产物名。
pub const OVM_ABI_SIDECAR_FILE: &str = "ovm-abi.json";

/// OVM 库导出 sidecar（`ovm-abi.json`）：合同 + 它所描述 `.odin` 产物的整体哈希。
///
/// 哈希覆盖产物**全部字节**，不挑字段计算：`.odin` 的布局表、常量池、代码段都
/// 参与 ABI 解释，挑字段迟早漏。宿主读取 sidecar 后必须先按整份产物重算哈希，
/// 对不上即失败闭合，禁止按未校验的布局解释内存。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiSidecar {
    /// 导出 ABI 合同。
    pub contract: OvmAbiContract,
    /// 产物文件名（例如 `app.odin`）。
    pub artifact: String,
    /// 产物整体哈希（`sha256-<hex>`）。
    pub artifact_sha256: String,
}

impl OvmAbiSidecar {
    /// 以产物全部字节冻结 sidecar；哈希算法与 [`Self::verify_against`] 唯一。
    pub fn new(contract: OvmAbiContract, artifact: String, artifact_bytes: &[u8]) -> Self {
        Self { contract, artifact, artifact_sha256: artifact_sha256(artifact_bytes) }
    }

    /// 用实际产物字节校验 sidecar；不一致即失败闭合。
    pub fn verify_against(&self, artifact_bytes: &[u8]) -> Result<()> {
        let actual = artifact_sha256(artifact_bytes);
        if actual != self.artifact_sha256 {
            return Err(format!(
                "OVM ABI sidecar 与产物 `{}` 哈希不一致：合同 `{}`，实际 `{actual}`；\
                 拒绝按未校验的 ABI 布局解释内存",
                self.artifact, self.artifact_sha256
            ));
        }
        Ok(())
    }
}

/// `.odin` 产物整体 SHA-256（`sha256-<hex>`）。
fn artifact_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("sha256-{hex}")
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
                &mut option_box_indices,
            )?);
        }
        let digest = contract_digest(&entries)?;
        let contract = Self { schema_version: OVM_ABI_SCHEMA_VERSION, lane: "ovm".to_string(), exports: entries, digest };
        // 合同必须与它所描述的 `.odin` 布局表一致；不一致 fail-closed。
        contract.verify_against_layouts(module_layouts)?;
        Ok(contract)
    }

    /// 按逻辑名查询导出合同。
    pub fn export(&self, logical_name: &str) -> Option<&OvmAbiExport> {
        self.exports.iter().find(|export| export.logical_name == logical_name)
    }

    /// 校验合同引用的每个布局下标都对应 `.odin` 的真实布局，且物理 recipe 可判。
    ///
    /// 这是“合同与运行时实际布局一致”的机械检查：
    /// - 托管文本的 `layout` / `field_count` / `byte_slot` 必须落在真实布局内；
    /// - 嵌套 Option 必须显式 box（`Niche` 会把 `Some(None)` 与 `None` 塌陷）；
    /// - box 布局必须是 2 槽且 `tag_slot` / `payload_slot` 为冻结配方；
    /// - 聚合 / 序列声明的字段数必须与真实布局一致。
    ///
    /// 任何一项不满足都失败闭合，禁止宿主按另一种解释继续。
    pub fn verify_against_layouts(&self, layouts: &[OdinLayout]) -> Result<()> {
        if self.schema_version != OVM_ABI_SCHEMA_VERSION {
            return Err(format!(
                "OVM ABI 合同 schema 版本 {} 与当前支持的 {OVM_ABI_SCHEMA_VERSION} 不一致",
                self.schema_version
            ));
        }
        for export in &self.exports {
            for param in &export.params {
                verify_type(&param.ty, layouts, &format!("导出 `{}` 参数 {}", export.logical_name, param.name))?;
            }
            verify_type(&export.returns, layouts, &format!("导出 `{}` 返回", export.logical_name))?;
        }
        Ok(())
    }
}

/// `utf8` 槽的文本表示事实来源。
#[derive(Clone, Copy, Debug)]
enum TextSlot {
    /// 入口形参槽；按 `position` 查逐槽冻结事实。
    Parameter(usize),
    /// 返回槽；只认 `return_layout`。
    Return,
    /// 嵌套槽（字段 / 元素 / 载荷）；没有逐槽冻结事实时显式落到 raw 宿主串。
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
/// 两处都没有 = 目标计划没有为该槽冻结托管布局，槽是 raw 宿主串（`None` 是
/// 事实，不是缺布局）。
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
    abi: &LibraryExportAbi,
    module_layouts: &mut Vec<OdinLayout>,
    layout_indices: &mut BTreeMap<LayoutId, i32>,
    sum_layout_indices: &BTreeMap<NominalInstanceId, i32>,
    option_box_indices: &mut BTreeMap<String, i32>,
) -> Result<OvmAbiExport> {
    let identities = canonical_parameter_identities(logical_name, function)?;
    if abi.params.len() != identities.len() {
        return Err(format!(
            "库导出 `{logical_name}` 冻结载体数（{}）与参数身份数（{}）不一致",
            abi.params.len(),
            identities.len()
        ));
    }
    let entry_layouts = frozen_entry_parameter_layouts(plan, function);
    let mut projection = Projection {
        plan,
        function,
        entry_layouts: &entry_layouts,
        module_layouts,
        layout_indices,
        sum_layout_indices,
        option_box_indices,
    };
    let mut params = Vec::with_capacity(identities.len());
    for (position, (_, name)) in identities.iter().enumerate() {
        let ty = function
            .param_types
            .get(position)
            .ok_or_else(|| format!("库导出 `{logical_name}` 参数 {position} 缺少类型事实"))?;
        let structure =
            abi_structure_for_type_with_layouts(ty, plan.aggregate_layouts(), plan.aggregate_layout_by_nyar_type(), plan.sum_layout_by_nyar_type());
        let projected = projection
            .project(&structure, TextSlot::Parameter(position))
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
    let returns = projection
        .project(&return_structure, TextSlot::Return)
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
                } else {
                    OvmOptionBoxing::Niche
                };
                OvmAbiType::Option { inner: Box::new(inner), boxing }
            }
            AbiStructure::Tuple { fields } => OvmAbiType::Tuple {
                fields: fields.iter().map(|field| self.project(field, TextSlot::Nested)).collect::<Result<Vec<_>>>()?,
            },
            AbiStructure::Aggregate(aggregate) => self.project_aggregate(aggregate)?,
            AbiStructure::Sequence(sequence) => self.project_sequence(sequence)?,
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
            AbiStructure::Unite { nominal, name, variants, .. } => {
                // 物理 sum 布局由 lowering 已登记的 nominal → 模块下标映射给出；
                // 缺失即失败闭合，绝不在合同里留“未知布局”。
                let layout = *self.sum_layout_indices.get(nominal).ok_or_else(|| {
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
    /// 每个槽的表示都是显式事实：
    /// - 入口参数只认 [`frozen_entry_parameter_layouts`] 的逐槽结果，且与 lowering
    ///   同源；若计划里已冻结 `utf8` 布局而该槽解析不到，说明合同与运行时布局
    ///   分叉，直接失败闭合；
    /// - 返回槽只认 `return_layout`（lowering 要求 `utf8` 返回必须是托管对象）；
    /// - 嵌套槽没有逐槽事实，显式落到 raw 宿主串，绝不回查全局布局猜测。
    fn project_text(&mut self, slot: TextSlot) -> Result<OvmAbiType> {
        let layout_id = match slot {
            TextSlot::Parameter(position) => {
                let layout = self.entry_layouts.get(position).copied().flatten();
                if layout.is_none() && self.plan.aggregate_layout_by_nyar_type().contains_key(&NyarType::Utf8) {
                    return Err(format!(
                        "导出参数 #{position} 的 `utf8` 槽没有解析到目标计划已冻结的文本布局：\
                         合同与运行时实际布局不一致，拒绝按 raw 串解释"
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
                ty: self.project(&field.ty, TextSlot::Nested).map_err(|reason| {
                    format!("聚合 `{namespace}::{layout_name}` 字段 `{}` 没有 OVM ABI 合同: {reason}", field.name)
                })?,
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

    fn project_sequence(&mut self, sequence: &SequenceStructure) -> Result<OvmAbiType> {
        let layout = find_layout(self.plan.aggregate_layouts(), sequence.layout)?;
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
            layout: layout_index(sequence.layout, layout.fields.len(), self.module_layouts, self.layout_indices)?,
            name: sequence.name.clone(),
            module_path: sequence.module_path.clone(),
            items_slot: sequence.items_slot,
            length_slot: sequence.length_slot,
            field_count: sequence.field_count,
            element: Box::new(self.project(&sequence.element, TextSlot::Nested)?),
            length_type: Box::new(self.project(&sequence.length_ty, TextSlot::Nested)?),
        })
    }
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

/// 合同结构对 `.odin` 布局表的机械校验（见 [`OvmAbiContract::verify_against_layouts`]）。
fn verify_type(ty: &OvmAbiType, layouts: &[OdinLayout], site: &str) -> Result<()> {
    let check_layout = |index: i32, expected: usize, role: &str| -> Result<()> {
        let position = usize::try_from(index).map_err(|_| format!("{site} 的 {role} 布局下标 `{index}` 为负"))?;
        let layout = layouts
            .get(position)
            .ok_or_else(|| format!("{site} 的 {role} 布局下标 `{index}` 超出 `.odin` 布局表（{} 项）", layouts.len()))?;
        if usize::try_from(layout.field_count) != Ok(expected) {
            return Err(format!(
                "{site} 的 {role} 声明字段数 {expected} 与 `.odin` 布局 `{index}` 的字段数 {} 不一致",
                layout.field_count
            ));
        }
        Ok(())
    };
    match ty {
        OvmAbiType::Unit
        | OvmAbiType::I32
        | OvmAbiType::I64
        | OvmAbiType::F32
        | OvmAbiType::F64
        | OvmAbiType::Bool
        | OvmAbiType::Char => Ok(()),
        OvmAbiType::Text { representation: OvmTextRepresentation::Raw } => Ok(()),
        OvmAbiType::Text { representation: OvmTextRepresentation::Managed { layout, field_count, byte_slot } } => {
            check_layout(*layout, *field_count, "托管文本")?;
            if *byte_slot >= *field_count {
                return Err(format!("{site} 的托管文本字节槽 {byte_slot} 超出声明字段数 {field_count}"));
            }
            Ok(())
        }
        OvmAbiType::Array { element } => verify_type(element, layouts, site),
        OvmAbiType::FixedArray { element, .. } => verify_type(element, layouts, site),
        OvmAbiType::Option { inner, boxing } => {
            if matches!(inner.as_ref(), OvmAbiType::Option { .. }) && matches!(boxing, OvmOptionBoxing::Niche) {
                return Err(format!(
                    "{site} 的嵌套 Option 使用 `Niche` 表示：`Some(None)` 与外层 `None` 会塌陷成同一物理值；\
                     嵌套 Option 必须显式 box（tag + payload 槽）"
                ));
            }
            if let OvmOptionBoxing::Boxed { layout, tag_slot, payload_slot } = boxing {
                check_layout(*layout, 2, "Option box")?;
                if *tag_slot != 0 || *payload_slot != 1 {
                    return Err(format!(
                        "{site} 的 Option box 配方必须是 tag_slot=0 / payload_slot=1，实际 tag_slot={tag_slot} / payload_slot={payload_slot}"
                    ));
                }
            }
            verify_type(inner, layouts, site)
        }
        OvmAbiType::Tuple { fields } => {
            for (index, field) in fields.iter().enumerate() {
                verify_type(field, layouts, &format!("{site}::tuple[{index}]"))?;
            }
            Ok(())
        }
        OvmAbiType::Aggregate { layout, fields, name, .. } => {
            check_layout(*layout, fields.len(), &format!("聚合 `{name}`"))?;
            for field in fields {
                verify_type(&field.ty, layouts, &format!("{site}::{}", field.name))?;
            }
            Ok(())
        }
        OvmAbiType::Sequence { layout, field_count, element, length_type, name, .. } => {
            check_layout(*layout, *field_count, &format!("序列 `{name}`"))?;
            verify_type(element, layouts, site)?;
            verify_type(length_type, layouts, site)
        }
        OvmAbiType::InstanceHandle { layout, name, .. } => {
            let position =
                usize::try_from(*layout).map_err(|_| format!("{site} 的引用实例 `{name}` 布局下标 `{layout}` 为负"))?;
            layouts
                .get(position)
                .ok_or_else(|| format!("{site} 的引用实例 `{name}` 布局下标 `{layout}` 超出 `.odin` 布局表"))?;
            Ok(())
        }
        OvmAbiType::Unite { layout, name, variants } => {
            let position = usize::try_from(*layout).map_err(|_| format!("{site} 的 unite `{name}` 布局下标 `{layout}` 为负"))?;
            layouts
                .get(position)
                .ok_or_else(|| format!("{site} 的 unite `{name}` 布局下标 `{layout}` 超出 `.odin` 布局表"))?;
            let mut tags = BTreeSet::new();
            for variant in variants {
                if !tags.insert(variant.tag) {
                    return Err(format!("{site} 的 unite `{name}` 变体 `{}` 判别 tag {} 重复", variant.name, variant.tag));
                }
            }
            Ok(())
        }
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
        let parameter_values = parameters
            .iter()
            .map(|(index, _, _)| ValueRef(u32::try_from(*index).expect("测试参数下标")))
            .collect::<Vec<_>>();
        ExecutableFunction {
            symbol: "exported".to_string(),
            return_type,
            param_types: parameters.iter().map(|(_, _, ty)| ty.clone()).collect(),
            value_types: parameters
                .iter()
                .map(|(index, _, ty)| (ValueRef(u32::try_from(*index).expect("测试参数下标")), ty.clone()))
                .collect(),
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
            plan.testing_wasm_export_names_mut().insert(instance, export.to_string());
        }
        plan
    }

    fn plan(export: &str, function: ExecutableFunction) -> TargetExecutionPlan {
        let mut plan = plan_with(export, function);
        plan.testing_freeze_library_export_abis().expect("载体冻结");
        plan
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
            plan.testing_wasm_export_names_mut().insert(instance, name.to_string());
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
        let sum_layout_indices = BTreeMap::new();
        let contract = OvmAbiContract::derive(plan, &export_entries(plan), &mut module_layouts, &mut layout_indices, &sum_layout_indices)?;
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
        let plan = plan("greet", function(&[(0, "name", ty)], nyar("unit")));
        let contract = derive(&plan).expect("`utf8` 必须有 OVM 合同");
        let export = export_of(&contract);
        assert_eq!(export.params[0].ty, OvmAbiType::Text { representation: OvmTextRepresentation::Raw });
        assert_eq!(export.params[0].carrier, "text");
        assert_eq!(export.returns, OvmAbiType::Unit);
        assert_eq!(export.returns_carrier, "void");
    }

    #[test]
    fn utf8_return_without_frozen_layout_fails_closed() {
        let ty = nyar("utf8");
        let plan = plan("greet", function(&[(0, "name", ty.clone())], ty));
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
        // 该槽自己的 `value_layouts` 事实，另一槽保持 raw。
        plan.testing_aggregate_layouts(nyar_types::AggregateLayoutPlan { layouts: vec![text_layout], ..Default::default() });
        for instance in plan.instances() {
            if plan.abi_name_for_instance(instance).map(|name| name.to_string()).as_deref() == Some("managed") {
                plan.testing_functions_mut()
                    .get_mut(&instance)
                    .expect("函数体")
                    .value_layouts
                    .insert(ValueRef(0), 11u32);
            }
        }
        plan.testing_freeze_library_export_abis().expect("载体冻结");
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
        let error = contract
            .verify_against_layouts(&[OdinLayout { field_count: 2 }])
            .expect_err("合同与运行时布局不一致必须失败闭合");
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
        assert_eq!(
            export_of(&contract).params[0].ty,
            OvmAbiType::Option { inner: Box::new(OvmAbiType::I64), boxing: OvmOptionBoxing::Niche }
        );
    }

    #[test]
    fn nested_option_projects_explicit_box_and_three_shapes_are_distinct() {
        let ty = NyarType::Nullable(Box::new(NyarType::Nullable(Box::new(nyar("i64")))));
        let plan = plan("maybe", function(&[(0, "value", ty.clone())], ty));
        let (contract, _) = derive_with_layouts(&plan).expect("`i64??` 必须有 OVM 合同");
        let returns = &export_of(&contract).returns;
        let OvmAbiType::Option { inner, boxing } = returns else {
            panic!("外层必须是 Option")
        };
        let OvmOptionBoxing::Boxed { layout, tag_slot, payload_slot } = *boxing else {
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
        let OvmAbiType::Option { boxing, .. } = &mut export_of_mut(&mut contract).returns else {
            panic!("外层必须是 Option")
        };
        *boxing = OvmOptionBoxing::Niche;
        let error = contract.verify_against_layouts(&layouts).expect_err("塌陷合同必须失败闭合");
        assert!(error.contains("塌陷"), "got: {error}");
    }

    /// 按合同编码 `None` / `Some(None)` / `Some(Some(x))`；只看合同，不看运行期类型。
    fn encode_option_shapes(ty: &OvmAbiType) -> [String; 3] {
        let OvmAbiType::Option { boxing, .. } = ty else {
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
