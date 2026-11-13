//! 目标私有物理布局解析。
//!
//! 语言 MIR 的 `AggregateLayout` 只携带语义事实：布局身份、字段顺序、完整类型与
//! 存储类；`offset/size/align` 是无目标占位。本模块按目标数据布局策略从这些
//! 语义事实计算物理 extent。任何后端读取 `offset/size/align` 前都必须先经过
//! 本解析器，不得消费语言层占位值，也不得在编码器内另写一套尺寸规则。

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldLayout, LayoutId, LayoutKey, NyarType};

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

/// 按策略计算聚合的物理字段偏移、字段宽度、对齐与整体尺寸。
///
/// 递归解析名义字段；环检测与缺失布局都失败闭合，禁止回落到默认尺寸。
pub(crate) fn resolve_physical_layouts(semantic: &AggregateLayoutPlan, policy: LayoutPolicy) -> Result<AggregateLayoutPlan, String> {
    let mut resolved = AggregateLayoutPlan {
        layouts: Vec::with_capacity(semantic.layouts.len()),
        synthetic_layouts: semantic.synthetic_layouts.clone(),
        declaration_to_layout: semantic.declaration_to_layout.clone(),
        value_type_declarations: semantic.value_type_declarations.clone(),
    };
    let mut memo: BTreeMap<LayoutId, (u32, u32)> = BTreeMap::new();
    for layout in &semantic.layouts {
        let extent = resolve_layout_extent(layout.id, semantic, policy, &mut memo, &mut BTreeSet::new())?;
        let mut fields = Vec::with_capacity(layout.fields.len());
        let mut offset = 0u32;
        for field in &layout.fields {
            let (size, align) = resolve_type_extent(&field.ty, semantic, policy, &mut memo, &mut BTreeSet::new())?;
            offset = align_up(offset, align);
            fields.push(FieldLayout { name: field.name.clone(), ty: field.ty.clone(), offset, size, align });
            offset = offset.saturating_add(size);
        }
        resolved.layouts.push(AggregateLayout {
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
    Ok(resolved)
}

fn resolve_layout_extent(
    id: LayoutId,
    semantic: &AggregateLayoutPlan,
    policy: LayoutPolicy,
    memo: &mut BTreeMap<LayoutId, (u32, u32)>,
    visiting: &mut BTreeSet<LayoutId>,
) -> Result<(u32, u32), String> {
    if let Some(extent) = memo.get(&id) {
        return Ok(*extent);
    }
    if !visiting.insert(id) {
        return Err(format!("聚合布局 `{id:?}` 的字段依赖形成环，无法计算目标物理尺寸"));
    }
    let layout = semantic
        .layouts
        .iter()
        .find(|candidate| candidate.id == id)
        .ok_or_else(|| format!("物理布局解析缺少布局身份 `{id:?}` 的语义记录"))?;
    // 零字段布局：值存储保留 1 字节最小载体；引用存储按引用槽。
    // 这是目标表示选择，不是语言层事实。
    if layout.fields.is_empty() {
        let extent = if layout.storage == nyar_types::StorageKind::Value {
            (1, 1)
        }
        else {
            (policy.reference_extent.0.max(1), policy.reference_extent.1.max(1))
        };
        visiting.remove(&id);
        memo.insert(id, extent);
        return Ok(extent);
    }
    let mut offset = 0u32;
    let mut max_align = 1u32;
    for field in &layout.fields {
        let (size, align) = resolve_type_extent(&field.ty, semantic, policy, memo, visiting)?;
        offset = align_up(offset, align);
        offset = offset.saturating_add(size);
        max_align = max_align.max(align);
    }
    visiting.remove(&id);
    let extent = (align_up(offset.max(1), max_align), max_align);
    memo.insert(id, extent);
    Ok(extent)
}

fn resolve_type_extent(
    ty: &NyarType,
    semantic: &AggregateLayoutPlan,
    policy: LayoutPolicy,
    memo: &mut BTreeMap<LayoutId, (u32, u32)>,
    visiting: &mut BTreeSet<LayoutId>,
) -> Result<(u32, u32), String> {
    match ty {
        NyarType::Nominal { layout, spelling, .. } => {
            resolve_layout_extent(*layout, semantic, policy, memo, visiting).map_err(|reason| format!("名义字段 `{spelling}`: {reason}"))
        }
        NyarType::Tuple(types) => {
            let key = LayoutKey::Tuple(types.clone());
            if let Some(layout) = semantic.synthetic_layouts.get(&key).copied() {
                return resolve_layout_extent(layout, semantic, policy, memo, visiting);
            }
            inline_synthetic_extent(types, policy, |element| resolve_type_extent(element, semantic, policy, memo, visiting))
        }
        NyarType::FixedArray { element, length } => {
            let key = LayoutKey::FixedArray { element: Box::new((**element).clone()), length: *length };
            if let Some(layout) = semantic.synthetic_layouts.get(&key).copied() {
                return resolve_layout_extent(layout, semantic, policy, memo, visiting);
            }
            inline_synthetic_extent(&vec![(**element).clone(); *length], policy, |element| {
                resolve_type_extent(element, semantic, policy, memo, visiting)
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
    use nyar_types::{FieldLayout, LayoutId, NyarType};

    fn semantic_layout(id: LayoutId, storage: nyar_types::StorageKind, fields: Vec<(&str, NyarType)>) -> AggregateLayout {
        AggregateLayout {
            id,
            name: format!("L{id}"),
            namespace: String::new(),
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

    fn plan(layouts: Vec<AggregateLayout>) -> AggregateLayoutPlan {
        AggregateLayoutPlan { layouts, ..Default::default() }
    }

    #[test]
    fn semantic_layout_becomes_target_physical_layout() {
        let nested = semantic_layout(2, nyar_types::StorageKind::Value, vec![("a", NyarType::Integer64 { signed: true })]);
        let outer = semantic_layout(
            1,
            nyar_types::StorageKind::Value,
            vec![("x", NyarType::Integer32 { signed: true }), ("nested", NyarType::Nominal {
                layout: 2,
                spelling: nyar_types::Identifier::new("Nested"),
            })],
        );
        let resolved = resolve_physical_layouts(&plan(vec![nested, outer]), LayoutPolicy::WASM32).expect("physical layout");
        let outer = resolved.layouts.iter().find(|layout| layout.id == 1).expect("outer");
        assert_eq!(outer.fields[0].offset, 0);
        assert_eq!(outer.fields[0].size, 4);
        // 嵌套值聚合按真实尺寸内联，不被固定引用槽宽度截断。
        assert_eq!(outer.fields[1].size, 8);
        assert_eq!(outer.size, 16);
    }

    #[test]
    fn integer128_requires_explicit_target_split() {
        let layout = semantic_layout(1, nyar_types::StorageKind::Value, vec![("wide", NyarType::Integer128 { signed: true })]);
        assert!(resolve_physical_layouts(&plan(vec![layout]), LayoutPolicy::WASM32).is_err());
    }

    #[test]
    fn zero_field_semantic_identity_is_not_a_physical_claim() {
        let layout = semantic_layout(1, nyar_types::StorageKind::Reference, Vec::new());
        let resolved = resolve_physical_layouts(&plan(vec![layout]), LayoutPolicy::WASM32).expect("physical layout");
        assert_eq!(resolved.layouts[0].size, 8);
        assert_eq!(resolved.layouts[0].align, 8);
    }
}
