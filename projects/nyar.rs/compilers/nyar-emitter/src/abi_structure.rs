//! 目标中立的结构化库导出 ABI schema。
//!
//! 本模块只描述**语义结构**：哪些类型可以跨越库导出边界，以及聚合 / 序列 /
//! sum 的内部结构（字段名、声明顺序槽位、判别 tag）。它不携带任何目标物理
//! 投影（wasm 导出名、OVM 布局下标），因此 wasm 与 ovm 两个后端都从同一份
//! 结构递归派生，禁止各自复制一份推导逻辑。
//!
//! 目标差异只留在投影层：
//! - wasm：结构 → `WasmAbiType`，附加 GC 字段 accessor / `struct.new` / 数组操作
//!   等 wasm 导出名（名字仍由冻结布局身份派生）。
//! - ovm：结构 → `OvmAbiType`，附加模块布局下标与无类型槽位事实。
//!
//! 结构派生规则（与 `freeze_abi_carrier` 冻结的载体合同一一对应）：
//! - 标量与 `utf8` 直接过界；
//! - `Array` / `FixedArray` / `Nullable` / `Tuple` / 具名聚合 / 泛型实例一律
//!   按冻结布局递归展开；
//! - `Integer128` / `Utf16` / `Bottom` / `Function` / `TraitObject` / `Union`
//!   与未注册布局都是 `Unsupported`，由各目标结构化失败。
//!
//! 禁止按类型名 / 包名 / 题目形状分支，也禁止缺布局时回落默认布局。

use std::collections::BTreeMap;

use nyar_types::{AggregateLayout, AggregateLayoutPlan, Identifier, LayoutId, NominalInstanceId, NyarType, StorageKind, SumTypeLayout};

use crate::Result;
use crate::backend_plan_views::ExecutableFunction;
use crate::contracts::ValueOrigin;

/// 目标中立的 ABI 语义结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiStructure {
    /// 无值（仅允许出现在返回位）。
    Unit,
    /// 32-bit 整数槽位。
    I32,
    /// 64-bit 整数槽位。
    I64,
    /// 32-bit 浮点槽位。
    F32,
    /// 64-bit 浮点槽位。
    F64,
    /// 布尔。
    Bool,
    /// Unicode 标量字符。
    Char,
    /// UTF-8 文本。
    Utf8,
    /// 堆数组 `[T]`。
    Array {
        /// 元素结构。
        element: Box<AbiStructure>,
        /// 元素类型的 canonical 拼写；只作为各目标派生物理名字的稳定身份种子，
        /// 不参与结构匹配。
        element_key: String,
    },
    /// 定长数组 `[T; N]`。
    FixedArray {
        /// 元素结构。
        element: Box<AbiStructure>,
        /// 元素数量。
        length: usize,
    },
    /// `T?` / Option。
    Option {
        /// 非空载荷结构。
        inner: Box<AbiStructure>,
    },
    /// 位置有语义的产品聚合。
    Tuple {
        /// 按声明顺序排列的字段结构。
        fields: Vec<AbiStructure>,
    },
    /// 具名 / 泛型聚合（值存储）。
    Aggregate(AggregateStructure),
    /// 冻结布局形状派生的序列容器视图（例如 `ArrayList<T>`）。
    Sequence(SequenceStructure),
    /// 引用存储名义实例的共享身份（不是字段快照）。
    InstanceHandle {
        /// 冻结布局身份。
        layout: LayoutId,
        /// 简单名。
        name: String,
        /// 完整命名空间路径。
        module_path: String,
    },
    /// 零 payload unite 的变体身份。
    Unite {
        /// 冻结 sum 名义实例身份（目标层据此解析物理 sum 布局）。
        nominal: NominalInstanceId,
        /// sum 简单名。
        name: String,
        /// 按声明顺序排列的变体。
        variants: Vec<AbiVariant>,
    },
    /// 不能出现在库导出边界、或缺少冻结布局事实的类型。
    Unsupported {
        /// 可读原因。
        reason: String,
    },
}

/// 具名聚合的结构描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateStructure {
    /// 冻结布局身份；身份模式（无布局事实）下为 `None`。
    pub layout: Option<LayoutId>,
    /// 简单名。
    pub name: String,
    /// 完整命名空间路径。
    pub module_path: String,
    /// 泛型实参结构；已由布局展开时为空。
    pub generic_args: Vec<AbiStructure>,
    /// 按声明顺序排列的字段；已由布局展开时携带完整字段合同。
    pub fields: Vec<AbiField>,
}

/// 聚合字段结构：字段名与字段类型结构。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiField {
    /// 字段名（来自冻结布局）。
    pub name: String,
    /// 字段结构。
    pub ty: AbiStructure,
}

/// 序列容器视图的共享结构事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceStructure {
    /// 容器冻结布局身份。
    pub layout: LayoutId,
    /// 容器简单名。
    pub name: String,
    /// 容器完整命名空间路径。
    pub module_path: String,
    /// backing array 字段的声明槽位。
    pub items_slot: usize,
    /// 计数字段的声明槽位。
    pub length_slot: usize,
    /// 容器字段总数。
    pub field_count: usize,
    /// backing array 元素结构。
    pub element: Box<AbiStructure>,
    /// 元素类型 canonical 拼写（目标派生物理名字的稳定身份种子）。
    pub element_key: String,
    /// 计数字段结构（整数）。
    pub length_ty: Box<AbiStructure>,
}

/// sum 变体身份：名字与判别 tag。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AbiVariant {
    /// 变体名（来自冻结 sum 布局）。
    pub name: String,
    /// 判别 tag。
    pub tag: u32,
}

impl AbiStructure {
    /// 返回结构化失败原因（若本结构不可跨库导出边界）。
    pub fn unsupported_reason(&self) -> Option<&str> {
        match self {
            Self::Unsupported { reason } => Some(reason),
            _ => None,
        }
    }
}

/// 把 `NyarType` 派生为纯身份结构（不读取任何布局事实）。
///
/// 该入口只用于“签名身份”场景：具名类型不展开字段，宿主拿到身份后必须
/// 查注册表或失败闭合，不能按名字猜字段。
pub fn abi_structure_for_type(ty: &NyarType) -> AbiStructure {
    match ty {
        NyarType::Bottom => AbiStructure::Unsupported { reason: "`void` / bottom 类型不能出现在库导出 ABI 位置".to_string() },
        NyarType::Unit => AbiStructure::Unit,

        NyarType::Boolean => AbiStructure::Bool,
        NyarType::Character => AbiStructure::Char,

        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } => AbiStructure::I32,
        NyarType::Integer64 { .. } => AbiStructure::I64,
        NyarType::Integer128 { .. } => {
            AbiStructure::Unsupported { reason: "`i128` 没有稳定的库导出载体，需要显式拆分合同".to_string() }
        }

        NyarType::Float32 => AbiStructure::F32,
        NyarType::Float64 => AbiStructure::F64,

        NyarType::Utf8 => AbiStructure::Utf8,
        NyarType::Utf16 => AbiStructure::Unsupported {
            reason: crate::lowering::features::physical_contract::UTF16_UNRESOLVED_TEXT_ABI.to_string(),
        },

        NyarType::Array(element) => {
            AbiStructure::Array { element: Box::new(abi_structure_for_type(element)), element_key: canonical_type_spelling(element) }
        }
        NyarType::FixedArray { element, length } => {
            AbiStructure::FixedArray { element: Box::new(abi_structure_for_type(element)), length: *length }
        }

        NyarType::Nullable(inner) => AbiStructure::Option { inner: Box::new(abi_structure_for_type(inner)) },

        NyarType::Tuple(fields) => AbiStructure::Tuple { fields: fields.iter().map(abi_structure_for_type).collect() },

        NyarType::Named(id) => identity_aggregate(simple_name_of(id), module_path_of(id), Vec::new()),
        NyarType::Nominal { spelling, .. } => identity_aggregate(simple_name_of(spelling), module_path_of(spelling), Vec::new()),

        NyarType::Apply(base, args) => {
            let (name, module_path, base_generic_args) = match base.as_ref() {
                NyarType::Named(id) => (simple_name_of(id), module_path_of(id), Vec::new()),
                NyarType::Nominal { spelling, .. } => (simple_name_of(spelling), module_path_of(spelling), Vec::new()),
                NyarType::Apply(_, _) => flatten_apply_identity(base.as_ref()),
                other => {
                    return AbiStructure::Unsupported {
                        reason: format!("Apply 的 Base 必须是 Named 或 Apply，实际是 {other:?}——无法提取语义 identity"),
                    };
                }
            };
            let mut generic_args: Vec<AbiStructure> = base_generic_args.iter().map(abi_structure_for_type).collect();
            generic_args.extend(args.iter().map(abi_structure_for_type));
            identity_aggregate(name, module_path, generic_args)
        }

        NyarType::Function(_) => AbiStructure::Unsupported { reason: "函数类型不能出现在库导出 ABI 位置".to_string() },

        NyarType::TraitObject(_) => {
            AbiStructure::Unsupported { reason: "TraitObject（dynamic dispatch）没有稳定的库导出载体".to_string() }
        }

        NyarType::Union(_) => AbiStructure::Unsupported { reason: "匿名 union 的库导出 ABI 合同尚未定义".to_string() },
    }
}

/// 携带冻结布局事实的结构派生：具名聚合按布局递归展开字段，`Apply` 按
/// `NyarType → LayoutId` 冻结索引回查布局，unite 按冻结 sum 布局给出变体身份。
///
/// 任一字段没有结构合同、或布局未注册时整个聚合为 `Unsupported`（结构化
/// 失败，不静默跳过字段），交给目标层拒绝。
pub fn abi_structure_for_type_with_layouts(
    ty: &NyarType,
    layouts: &AggregateLayoutPlan,
    layout_by_type: &BTreeMap<NyarType, LayoutId>,
    sums: &BTreeMap<NyarType, SumTypeLayout>,
) -> AbiStructure {
    // unite 没有物理布局身份：只有零 payload unite 才有纯变体身份合同；
    // 带 payload 的 sum 仍回落具名聚合身份，由目标层在边界失败闭合。
    if let Some(sum) = sums.get(ty) {
        if sum.is_unite && sum.variants.iter().all(|variant| variant.payload_type.is_none()) {
            return AbiStructure::Unite {
                nominal: sum.nominal,
                name: sum.name.clone(),
                variants: sum.variants.iter().map(|variant| AbiVariant { name: variant.name.clone(), tag: variant.tag }).collect(),
            };
        }
    }
    match ty {
        NyarType::Nominal { layout, spelling } => {
            aggregate_structure(simple_name_of(spelling), module_path_of(spelling), *layout, layouts, layout_by_type, sums)
        }
        // `Apply` 不携带 `LayoutId`；布局身份只能来自 canonical 计划冻结的
        // `NyarType → LayoutId` 索引。索引里没有就退回身份结构，目标层据此
        // 结构化失败，绝不按名字猜布局。
        NyarType::Apply(..) => match layout_by_type.get(ty) {
            Some(layout) => {
                let (name, module_path, _) = flatten_apply_identity(ty);
                aggregate_structure(name, module_path, *layout, layouts, layout_by_type, sums)
            }
            None => abi_structure_for_type(ty),
        },
        NyarType::Array(element) => AbiStructure::Array {
            element: Box::new(abi_structure_for_type_with_layouts(element, layouts, layout_by_type, sums)),
            element_key: canonical_type_spelling(element),
        },
        NyarType::FixedArray { element, length } => AbiStructure::FixedArray {
            element: Box::new(abi_structure_for_type_with_layouts(element, layouts, layout_by_type, sums)),
            length: *length,
        },
        NyarType::Nullable(inner) => AbiStructure::Option {
            inner: Box::new(abi_structure_for_type_with_layouts(inner, layouts, layout_by_type, sums)),
        },
        NyarType::Tuple(fields) => AbiStructure::Tuple {
            fields: fields.iter().map(|field| abi_structure_for_type_with_layouts(field, layouts, layout_by_type, sums)).collect(),
        },
        other => abi_structure_for_type(other),
    }
}

fn identity_aggregate(name: String, module_path: String, generic_args: Vec<AbiStructure>) -> AbiStructure {
    AbiStructure::Aggregate(AggregateStructure { layout: None, name, module_path, generic_args, fields: Vec::new() })
}

/// 已知布局身份的聚合结构。
///
/// 序列形状（一个 heap array 字段 + 一个整数计数字段、Reference 存储）投影为
/// 序列视图；其余 Reference 存储名义实例是共享身份；Value 存储聚合保留字段
/// 结构。三者都由冻结布局事实派生，不看类型名。
fn aggregate_structure(
    name: String,
    module_path: String,
    layout_id: LayoutId,
    layouts: &AggregateLayoutPlan,
    layout_by_type: &BTreeMap<NyarType, LayoutId>,
    sums: &BTreeMap<NyarType, SumTypeLayout>,
) -> AbiStructure {
    let Some(aggregate) = layouts.layouts.iter().find(|item| item.id == layout_id) else {
        return AbiStructure::Unsupported {
            reason: format!("layout `{layout_id}` 未注册，无法派生字段访问合同"),
        };
    };
    if let Some(sequence) = sequence_structure(aggregate, name.clone(), module_path.clone(), layouts, layout_by_type, sums) {
        return sequence;
    }
    // 引用存储的非序列名义实例是共享身份，不是字段快照：宿主必须拿到同一
    // 引用，按字段复制会让 `mut self` 方法改在副本上。
    if aggregate.storage == StorageKind::Reference {
        return AbiStructure::InstanceHandle { layout: layout_id, name, module_path };
    }
    let mut fields = Vec::with_capacity(aggregate.fields.len());
    for field in &aggregate.fields {
        let field_structure = abi_structure_for_type_with_layouts(&field.ty, layouts, layout_by_type, sums);
        if let Some(reason) = field_structure.unsupported_reason() {
            return AbiStructure::Unsupported {
                reason: format!("聚合 `{}::{}` 字段 `{}` 没有 ABI 合同: {reason}", aggregate.namespace, aggregate.name, field.name),
            };
        }
        fields.push(AbiField { name: field.name.clone(), ty: field_structure });
    }
    AbiStructure::Aggregate(AggregateStructure { layout: Some(layout_id), name, module_path, generic_args: Vec::new(), fields })
}

/// 从冻结布局形状派生序列视图。
///
/// 条件是纯结构的：`Reference` 存储、恰好两个字段、一个 heap `Array<T>` 字段
/// 和一个整数字段。满足时 backing array 与计数槽位都来自布局事实；不满足时
/// 返回 `None`，绝不按类型名特判 `ArrayList`。
fn sequence_structure(
    aggregate: &AggregateLayout,
    name: String,
    module_path: String,
    layouts: &AggregateLayoutPlan,
    layout_by_type: &BTreeMap<NyarType, LayoutId>,
    sums: &BTreeMap<NyarType, SumTypeLayout>,
) -> Option<AbiStructure> {
    if aggregate.storage != StorageKind::Reference || aggregate.fields.len() != 2 {
        return None;
    }
    let items_slot = aggregate.fields.iter().position(|field| matches!(field.ty, NyarType::Array(_)))?;
    let length_slot = aggregate.fields.iter().position(|field| is_integer_type(&field.ty))?;
    if items_slot == length_slot {
        return None;
    }
    let NyarType::Array(element) = &aggregate.fields[items_slot].ty else {
        return None;
    };
    let element_structure = abi_structure_for_type_with_layouts(element, layouts, layout_by_type, sums);
    if let Some(reason) = element_structure.unsupported_reason() {
        return Some(AbiStructure::Unsupported {
            reason: format!("序列容器 `{}::{}` 的元素类型没有 ABI 合同: {reason}", aggregate.namespace, aggregate.name),
        });
    }
    let length_ty = abi_structure_for_type_with_layouts(&aggregate.fields[length_slot].ty, layouts, layout_by_type, sums);
    Some(AbiStructure::Sequence(SequenceStructure {
        layout: aggregate.id,
        name,
        module_path,
        items_slot,
        length_slot,
        field_count: aggregate.fields.len(),
        element: Box::new(element_structure),
        element_key: canonical_type_spelling(element),
        length_ty: Box::new(length_ty),
    }))
}

fn is_integer_type(ty: &NyarType) -> bool {
    matches!(
        ty,
        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } | NyarType::Integer64 { .. }
    )
}

/// 类型 canonical 拼写：目标层派生数组物理名字的稳定身份种子。
///
/// 只由类型结构决定，不看模块名或题目形状。
pub fn canonical_type_spelling(ty: &NyarType) -> String {
    format!("{ty:?}")
}

/// sum 身份事实：类型名、是否 unite、全部变体名与判别 tag。
pub fn sum_identity_facts(name: &str, is_unite: bool, variants: &[AbiVariant]) -> String {
    let mut facts = String::new();
    facts.push_str(name);
    facts.push('\n');
    facts.push_str(if is_unite { "unite" } else { "sum" });
    facts.push('\n');
    for variant in variants {
        facts.push_str(&format!("{}:{}\n", variant.name, variant.tag));
    }
    facts
}

/// 从冻结 `SumTypeLayout` 提取结构身份。
pub fn sum_variants_of(sum: &SumTypeLayout) -> Vec<AbiVariant> {
    sum.variants.iter().map(|variant| AbiVariant { name: variant.name.clone(), tag: variant.tag }).collect()
}

/// 从函数 SSA 值提取入口参数身份 `(声明下标, canonical 参数名)`。
///
/// 身份必须来自 `ValueOrigin::Parameter`，数量必须与 `param_types` 一致，下标
/// 必须连续且从 0 开始，参数名必须非空且不重复；任何偏离都在此结构化失败。
/// wasm 与 ovm 的库导出合同共用这一份校验。
pub(crate) fn canonical_parameter_identities<'a>(instance_label: &str, function: &'a ExecutableFunction) -> Result<Vec<(usize, &'a str)>> {
    let mut identities = function
        .values
        .iter()
        .filter_map(|value| match &value.origin {
            ValueOrigin::Parameter { index, name } => Some((*index, name.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    if identities.len() != function.param_types.len() {
        return Err(format!(
            "库导出 `{instance_label}` 的入口参数身份数量与参数类型数量不一致: parameter_origins={} param_types={}",
            identities.len(),
            function.param_types.len()
        ));
    }
    identities.sort_by_key(|(index, _)| *index);
    let mut names = std::collections::BTreeSet::new();
    for (position, (index, name)) in identities.iter().enumerate() {
        if *index != position {
            return Err(format!("库导出 `{instance_label}` 参数下标不连续: position={position} index={index}"));
        }
        if name.trim().is_empty() {
            return Err(format!("库导出 `{instance_label}` 参数 {index} 缺少 canonical 参数名"));
        }
        if !names.insert(*name) {
            return Err(format!("库导出 `{instance_label}` 参数 {index} 的参数名 `{name}` 重复"));
        }
    }
    Ok(identities)
}

/// 从 `Identifier` 提取简单名（丢掉模块路径前缀）。
pub fn simple_name_of(id: &Identifier) -> String {
    let full = id.as_str();
    full.rsplit_once("::")
        .map(|(_, last)| last.to_string())
        .or_else(|| full.rsplit_once('.').map(|(_, last)| last.to_string()))
        .unwrap_or_else(|| full.to_string())
}

/// 从 `Identifier` 提取模块路径（丢掉最终简单名）。
///
/// - `"std::collections::ArrayList"` → `"std::collections"`
/// - `"std.collections.ArrayList"` → `"std.collections"`
/// - `"ArrayList"` → `""`（简单名无路径前缀）
///
/// 空返回值只在**上游如实无路径事实**时出现：canonical 已为每个 struct 声明
/// 登记 `MirStruct::qualified_name()`；不要用简单名拼 package path 来填补空值。
pub fn module_path_of(id: &Identifier) -> String {
    let full = id.as_str();
    if let Some((prefix, _)) = full.rsplit_once("::") {
        prefix.to_string()
    }
    else if let Some((prefix, _)) = full.rsplit_once('.') {
        prefix.to_string()
    }
    else {
        String::new()
    }
}

/// 把嵌套的 `Apply(Apply(Named("A"), ...), ...)` 展平，提取最顶层语义名、
/// 模块路径与最底层 generic_args 残留（极罕见场景）。
fn flatten_apply_identity(mut ty: &NyarType) -> (String, String, Vec<NyarType>) {
    let mut accumulated: Vec<NyarType> = Vec::new();
    loop {
        match ty {
            NyarType::Named(id) => return (simple_name_of(id), module_path_of(id), accumulated),
            NyarType::Nominal { spelling, .. } => return (simple_name_of(spelling), module_path_of(spelling), accumulated),
            NyarType::Apply(base, args) => {
                accumulated.extend(args.iter().cloned());
                ty = base.as_ref();
            }
            other => {
                return (format!("<unexpected:{other:?}>"), String::new(), accumulated);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::contracts::{Block, BlockRef, ExecutableFunction, Terminator, Value, ValueOrigin, ValueRef};

    use super::*;

    fn nyar(s: &str) -> NyarType {
        NyarType::from_surface_primitive(s).expect("surface primitive")
    }

    #[allow(deprecated)]
    fn identity_function(parameters: &[(usize, &str, NyarType)], return_type: NyarType) -> ExecutableFunction {
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
                    id: ValueRef(u32::try_from(*index).expect("test parameter index")),
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

    #[test]
    fn scalars_keep_semantic_identity() {
        assert_eq!(abi_structure_for_type(&NyarType::Unit), AbiStructure::Unit);
        assert_eq!(abi_structure_for_type(&nyar("bool")), AbiStructure::Bool);
        assert_eq!(abi_structure_for_type(&nyar("i32")), AbiStructure::I32);
        assert_eq!(abi_structure_for_type(&nyar("i64")), AbiStructure::I64);
        assert_eq!(abi_structure_for_type(&nyar("f64")), AbiStructure::F64);
        assert_eq!(abi_structure_for_type(&nyar("utf8")), AbiStructure::Utf8);
    }

    #[test]
    fn unsupported_scalars_fail_closed() {
        for ty in [nyar("i128"), NyarType::Utf16] {
            assert!(abi_structure_for_type(&ty).unsupported_reason().is_some(), "`{ty}` 必须结构化失败");
        }
    }

    #[test]
    fn identity_aggregate_does_not_guess_fields() {
        let ty = NyarType::Apply(Box::new(NyarType::Named(Identifier::new("std::collections::ArrayList"))), vec![nyar("i64")]);
        let AbiStructure::Aggregate(aggregate) = abi_structure_for_type(&ty) else {
            panic!("Apply 必须有身份结构")
        };
        assert_eq!(aggregate.name, "ArrayList");
        assert_eq!(aggregate.module_path, "std::collections");
        assert_eq!(aggregate.generic_args, vec![AbiStructure::I64]);
        assert!(aggregate.fields.is_empty(), "没有布局事实时不得猜字段");
        assert_eq!(aggregate.layout, None);
    }

    #[test]
    fn canonical_parameter_identities_reject_duplicates() {
        let function = identity_function(&[(0, "value", nyar("i64")), (1, "value", nyar("i64"))], nyar("i64"));
        let error = canonical_parameter_identities("identity", &function).expect_err("重复参数名必须失败");
        assert!(error.contains("重复"), "got: {error}");
    }
}
