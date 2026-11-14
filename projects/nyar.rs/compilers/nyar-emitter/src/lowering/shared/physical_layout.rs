//! 目标物理布局解析。
//!
//! 语言层只提交语义聚合形状（字段身份、类型、顺序、值/引用存储类）。
//! 本模块按目标自己的 [`LayoutPolicy`] 从这些语义事实计算字段偏移、字段宽度、
//! 对齐与对象 body 尺寸，并产出**独立**的 [`ResolvedLayoutPlan`]：物理解析结果
//! 与语义计划是两组类型，消费者无法再把语义形状当成已布局事实。
//!
//! 引用存储的聚合与引用类型的字段只占引用槽；只有值存储聚合才按 body 内联递归。

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{AggregateLayoutPlan, LayoutId, LayoutKey, NyarType, StorageKind};

/// 目标数据布局策略。
///
/// 只包含目标自己的物理事实：引用槽宽度、文本槽宽度与标量宽度规则。
/// 语言层不得选择这些值。
#[derive(Debug, Clone, Copy)]
pub(crate) struct LayoutPolicy {
    /// 引用/句柄槽的字节宽度与对齐。
    pub reference_extent: (u32, u32),
    /// 文本槽的字节宽度与对齐。
    pub text_extent: (u32, u32),
}

impl LayoutPolicy {
    /// Wasm32 线性内存策略。
    ///
    /// 引用与文本槽沿用现有 Wasm lane 的 8 字节槽宽；这是目标选择，
    /// 不是语言事实。
    pub(crate) const WASM32: Self = Self { reference_extent: (8, 8), text_extent: (4, 4) };

    /// Odin VM 策略。
    pub(crate) const OVM: Self = Self { reference_extent: (8, 8), text_extent: (4, 4) };
}

/// 已解析的字段物理布局。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedFieldLayout {
    /// 字段身份（来自语义布局）。
    pub name: String,
    /// 字段类型（来自语义布局）。
    pub ty: NyarType,
    /// 相对对象 body 起点的字节偏移。
    pub offset: u32,
    /// 字段在对象 body 中占用的字节数。
    pub size: u32,
    /// 字段对齐。
    pub align: u32,
}

/// 已解析的聚合物理布局。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedAggregateLayout {
    /// 布局身份（与语义计划一致）。
    pub id: LayoutId,
    /// 类型简单名（诊断用）。
    pub name: String,
    /// 命名空间（诊断用）。
    pub namespace: String,
    /// 合成布局键；名义布局为 `None`。
    pub key: Option<LayoutKey>,
    /// 值/引用存储类。
    pub storage: StorageKind,
    /// 对象 body 的总尺寸。
    pub size: u32,
    /// 聚合对齐。
    pub align: u32,
    /// 已解析字段。
    pub fields: Vec<ResolvedFieldLayout>,
}

/// 目标已解析的模块级布局计划。
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedLayoutPlan {
    /// 按布局身份升序的物理布局。
    pub layouts: Vec<ResolvedAggregateLayout>,
    /// 合成 tuple / fixedarray 结构键到布局 id 的映射。
    pub synthetic_layouts: BTreeMap<LayoutKey, LayoutId>,
    /// 结构声明到布局的稳定绑定。
    pub declaration_to_layout: BTreeMap<nyar_types::ItemId, LayoutId>,
    /// 按 declaration 登记的 value 存储结构。
    pub value_type_declarations: BTreeSet<nyar_types::ItemId>,
}

impl ResolvedLayoutPlan {
    /// 按布局身份查询物理布局。
    pub fn layout(&self, id: LayoutId) -> Option<&ResolvedAggregateLayout> {
        self.layouts.iter().find(|layout| layout.id == id)
    }
}

/// 按策略计算聚合的物理字段偏移、字段宽度、对齐与整体尺寸。
///
/// 递归解析值存储的名义字段；引用存储的聚合与引用类型字段只占引用槽，
/// 不展开 body。环检测与缺失布局都失败闭合，禁止回落到默认尺寸。
pub(crate) fn resolve_physical_layouts(semantic: &AggregateLayoutPlan, policy: LayoutPolicy) -> Result<ResolvedLayoutPlan, String> {
    let mut memo: BTreeMap<LayoutId, (u32, u32)> = BTreeMap::new();
    let mut layouts = Vec::with_capacity(semantic.layouts.len());
    for layout in &semantic.layouts {
        let extent = layout_extent(layout.id, semantic, policy, &mut memo, &mut BTreeSet::new())?;
        let mut fields = Vec::with_capacity(layout.fields.len());
        let mut offset = 0u32;
        for field in &layout.fields {
            let (size, align) = type_extent(&field.ty, semantic, policy, &mut memo, &mut BTreeSet::new())?;
            offset = align_up(offset, align);
            fields.push(ResolvedFieldLayout { name: field.name.clone(), ty: field.ty.clone(), offset, size, align });
            offset = offset.saturating_add(size);
        }
        layouts.push(ResolvedAggregateLayout {
            id: layout.id,
            name: layout.name.clone(),
            namespace: layout.namespace.clone(),
            key: layout.key.clone(),
            storage: layout.storage,
            size: extent.0,
            align: extent.1,
            fields,
        });
    }
    Ok(ResolvedLayoutPlan {
        layouts,
        synthetic_layouts: semantic.synthetic_layouts.clone(),
        declaration_to_layout: semantic.declaration_to_layout.clone(),
        value_type_declarations: semantic.value_type_declarations.clone(),
    })
}

fn layout_extent(
    id: LayoutId,
    semantic: &AggregateLayoutPlan,
    policy: LayoutPolicy,
    memo: &mut BTreeMap<LayoutId, (u32, u32)>,
    visiting: &mut BTreeSet<LayoutId>,
) -> Result<(u32, u32), String> {
    if let Some(extent) = memo.get(&id) {
        return Ok(*extent);
    }
    let layout =
        semantic.layouts.iter().find(|candidate| candidate.id == id).ok_or_else(|| format!("物理布局解析缺少布局身份 `{id:?}` 的语义记录"))?;
    // 引用存储的对象由目标用引用槽/句柄承载，不内联 body，也不参与值递归。
    if layout.storage == StorageKind::Reference {
        memo.insert(id, policy.reference_extent);
        return Ok(policy.reference_extent);
    }
    if !visiting.insert(id) {
        return Err(format!("聚合布局 `{id:?}` 的字段依赖形成环，无法计算目标物理尺寸"));
    }
    // 零字段值类型保留 1 字节最小载体；这是目标表示选择，不是语言层事实。
    if layout.fields.is_empty() {
        visiting.remove(&id);
        memo.insert(id, (1, 1));
        return Ok((1, 1));
    }
    let mut offset = 0u32;
    let mut max_align = 1u32;
    for field in &layout.fields {
        let (size, align) = type_extent(&field.ty, semantic, policy, memo, visiting)?;
        offset = align_up(offset, align);
        offset = offset.saturating_add(size);
        max_align = max_align.max(align);
    }
    visiting.remove(&id);
    let extent = (align_up(offset.max(1), max_align), max_align);
    memo.insert(id, extent);
    Ok(extent)
}

fn type_extent(
    ty: &NyarType,
    semantic: &AggregateLayoutPlan,
    policy: LayoutPolicy,
    memo: &mut BTreeMap<LayoutId, (u32, u32)>,
    visiting: &mut BTreeSet<LayoutId>,
) -> Result<(u32, u32), String> {
    match ty {
        NyarType::Nominal { layout, spelling, .. } => {
            layout_extent(*layout, semantic, policy, memo, visiting).map_err(|reason| format!("名义字段 `{spelling}`: {reason}"))
        }
        NyarType::Tuple(types) => {
            let key = LayoutKey::Tuple(types.clone());
            if let Some(layout) = semantic.synthetic_layouts.get(&key).copied() {
                return layout_extent(layout, semantic, policy, memo, visiting);
            }
            inline_synthetic_extent(types, policy, |element| type_extent(element, semantic, policy, memo, visiting))
        }
        NyarType::FixedArray { element, length } => {
            let key = LayoutKey::FixedArray { element: Box::new((**element).clone()), length: *length };
            if let Some(layout) = semantic.synthetic_layouts.get(&key).copied() {
                return layout_extent(layout, semantic, policy, memo, visiting);
            }
            inline_synthetic_extent(&vec![(**element).clone(); *length], policy, |element| {
                type_extent(element, semantic, policy, memo, visiting)
            })
        }
        NyarType::Boolean
        | NyarType::Character
        | NyarType::Integer8 { .. }
        | NyarType::Integer16 { .. }
        | NyarType::Integer32 { .. }
        | NyarType::Float32
        | NyarType::Bottom
        | NyarType::Unit => Ok((4, 4)),
        NyarType::Integer64 { .. } | NyarType::Float64 => Ok((8, 8)),
        NyarType::Utf8 | NyarType::Utf16 => Ok(policy.text_extent),
        NyarType::Array(_) | NyarType::Nullable(_) | NyarType::Named(_) | NyarType::Apply(_, _) | NyarType::Function(_) => {
            Ok(policy.reference_extent)
        }
        NyarType::Union(_) | NyarType::TraitObject(_) => Ok(policy.reference_extent),
        NyarType::Integer128 { .. } => Err("`i128/u128` 没有目标字段宽度，目标必须显式拆分".to_string()),
    }
}

fn inline_synthetic_extent(
    element_types: &[NyarType],
    _policy: LayoutPolicy,
    mut extent_for: impl FnMut(&NyarType) -> Result<(u32, u32), String>,
) -> Result<(u32, u32), String> {
    let mut offset = 0u32;
    let mut max_align = 1u32;
    for element in element_types {
        let (size, align) = extent_for(element)?;
        offset = align_up(offset, align);
        offset = offset.saturating_add(size);
        max_align = max_align.max(align);
    }
    Ok((align_up(offset.max(1), max_align), max_align))
}

fn align_up(offset: u32, align: u32) -> u32 {
    let align = align.max(1);
    offset.div_ceil(align) * align
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar_types::{AggregateLayout, FieldLayout};

    fn semantic_layout(id: LayoutId, storage: StorageKind, fields: Vec<(&str, NyarType)>) -> AggregateLayout {
        AggregateLayout {
            id,
            name: format!("L{id}"),
            namespace: String::new(),
            key: None,
            storage,
            fields: fields.into_iter().map(|(name, ty)| FieldLayout { name: name.to_string(), ty }).collect(),
        }
    }

    fn plan(layouts: Vec<AggregateLayout>) -> AggregateLayoutPlan {
        AggregateLayoutPlan { layouts, ..Default::default() }
    }

    #[test]
    fn semantic_layout_becomes_target_physical_layout() {
        let nested = semantic_layout(2, StorageKind::Value, vec![("a", NyarType::Integer64 { signed: true })]);
        let outer = semantic_layout(
            1,
            StorageKind::Value,
            vec![
                ("x", NyarType::Integer32 { signed: true }),
                ("nested", NyarType::Nominal { layout: 2, spelling: nyar_types::Identifier::new("Nested") }),
            ],
        );
        let resolved = resolve_physical_layouts(&plan(vec![nested, outer]), LayoutPolicy::WASM32).expect("physical layout");
        let outer = resolved.layout(1).expect("outer");
        assert_eq!(outer.fields[0].offset, 0);
        assert_eq!(outer.fields[0].size, 4);
        // 嵌套值聚合按真实尺寸内联，不被固定引用槽宽度截断。
        assert_eq!(outer.fields[1].size, 8);
        assert_eq!(outer.size, 16);
    }

    /// 引用存储的名义字段只占引用槽，不展开对象 body；直接自引用必须可解析。
    #[test]
    fn reference_storage_field_is_a_slot_not_inlined_body() {
        let node = semantic_layout(
            1,
            StorageKind::Reference,
            vec![
                ("next", NyarType::Nominal { layout: 1, spelling: nyar_types::Identifier::new("Node") }),
                ("value", NyarType::Integer64 { signed: true }),
            ],
        );
        let resolved = resolve_physical_layouts(&plan(vec![node]), LayoutPolicy::WASM32).expect("自引用引用布局必须可解析");
        let node = resolved.layout(1).expect("node");
        assert_eq!(node.fields[0].size, 8, "引用字段占引用槽");
        assert_eq!(node.size, 8);
    }

    #[test]
    fn integer128_requires_explicit_target_split() {
        let layout = semantic_layout(1, StorageKind::Value, vec![("wide", NyarType::Integer128 { signed: true })]);
        assert!(resolve_physical_layouts(&plan(vec![layout]), LayoutPolicy::WASM32).is_err());
    }

    #[test]
    fn zero_field_value_identity_gets_minimal_target_carrier() {
        let layout = semantic_layout(1, StorageKind::Value, Vec::new());
        let resolved = resolve_physical_layouts(&plan(vec![layout]), LayoutPolicy::WASM32).expect("physical layout");
        assert_eq!(resolved.layout(1).expect("layout").size, 1);
    }
}
