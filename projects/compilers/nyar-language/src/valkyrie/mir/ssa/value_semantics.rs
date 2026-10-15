//! Value vs reference storage classification for MIR aggregates.
//!
//! `structure`, tuple, and fixed array must stay **value** (`MirStorageKind::Value`):
//! inline layout, by-value move/copy, stack or linear-memory slots — never GC heap objects.
//! `class` and heap `[T]` remain **reference** (`MirStorageKind::Reference`).
//!
//! Layout payload types live in `nyar_types`; this module owns Valkyrie HIR → layout planning.

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::StorageKind;

use crate::{
    frontend_contract::concretize_type,
    types::{
        Identifier,
        hir::{HirModule, HirStruct, ValkyrieType},
    },
};

pub use nyar_types::{
    AggregateLayout, AggregateLayoutPlan, FieldLayout, FlagsLayout, LayoutId, SumTypeLayout, SumVariantLayout, layout_key_for_nyar_type,
};

/// Physical storage class for aggregate values at lowering time.
///
/// Type alias to the platform [`StorageKind`] so MIR and backends share one enum.
pub type MirStorageKind = StorageKind;

pub fn storage_kind_for_named_type(type_name: &str, struct_is_value_type: &BTreeMap<String, bool>) -> MirStorageKind {
    match struct_is_value_type.get(type_name) {
        Some(true) => MirStorageKind::Value,
        Some(false) | None => MirStorageKind::Reference,
    }
}

pub fn storage_kind_for_type(ty: &ValkyrieType, value_type_names: &BTreeSet<Identifier>) -> MirStorageKind {
    match ty {
        ValkyrieType::Tuple(_) | ValkyrieType::FixedArray { .. } => MirStorageKind::Value,
        ValkyrieType::Array(_) => MirStorageKind::Reference,
        ValkyrieType::Named(name) if value_type_names.contains(name) => MirStorageKind::Value,
        _ => MirStorageKind::Reference,
    }
}

pub fn value_type_names_from_module(structs: &[HirStruct]) -> BTreeSet<Identifier> {
    structs.iter().filter(|item| item.is_value_type).map(|item| item.name.clone()).collect()
}

pub fn layout_key_for_type(ty: &ValkyrieType) -> Option<String> {
    // 仅合成 tuple / fixedarray 键；名义类型必须走 declaration_to_layout，禁止 Named 短名键。
    match ty {
        ValkyrieType::Tuple(types) => Some(format!("__tuple_{}", types.iter().map(type_layout_key_component).collect::<Vec<_>>().join("_"))),
        ValkyrieType::FixedArray { element, length } => Some(format!("__fixedarray_{length}_{}", type_layout_key_component(element))),
        _ => None,
    }
}

pub fn layout_id_for_type(ty: &ValkyrieType, plan: &AggregateLayoutPlan) -> Option<LayoutId> {
    // The HIR normalizer may represent a nominal aggregate as a one-element
    // tuple. Preserve the nominal layout identity when it is registered; using
    // the synthetic tuple layout would expose fields `0`, `1`, ... to semantic
    // FieldGet and lose the aggregate's declared field metadata.
    if let ValkyrieType::Tuple(elements) = ty {
        // Only the HIR normalizer's one-element wrapper can reuse a nominal
        // layout.  A real tuple keeps tuple identity even when all elements
        // happen to share the same nominal layout; otherwise `(T, T)` is
        // incorrectly resolved as `T` and tuple_get loses its field metadata.
        if elements.len() == 1 {
            let mut nominal_id = None;
            let mut consistent = true;
            for element in elements {
                let Some(id) = layout_id_for_type(element, plan)
                else {
                    consistent = false;
                    break;
                };
                if let Some(existing) = nominal_id {
                    if existing != id {
                        consistent = false;
                        break;
                    }
                }
                else {
                    nominal_id = Some(id);
                }
            }
            if consistent && nominal_id.is_some() {
                return nominal_id;
            }
        }
    }
    // 生产查询只认 declaration → layout；名义缺布局不得回落 Named/限定名字符串。
    if let Some(declaration) = ty.nominal_declaration() {
        return plan.declaration_to_layout.get(&declaration).copied();
    }
    // 合成 tuple / fixedarray 键；禁止 Future/Promise 等短名透明解包。
    let key = layout_key_for_type(ty)?;
    plan.type_name_to_layout.get(&key).copied()
}

pub fn compute_aggregate_layout_plan(module: &HirModule) -> AggregateLayoutPlan {
    let mut plan = AggregateLayoutPlan::default();
    let mut structs = Vec::new();
    collect_module_structs(module, &mut structs);
    for hir_struct in structs {
        // 开放泛型声明不得生成平台布局（字段仍是 binder）；与 SumTypeLayout 同口径。
        if !hir_struct.generics.is_empty() {
            continue;
        }
        let storage = if hir_struct.is_value_type { MirStorageKind::Value } else { MirStorageKind::Reference };
        let layout = layout_for_struct(hir_struct, &mut plan, storage);
        register_declared_layout(&mut plan, hir_struct, layout);
    }
    // Semantic-group builds keep only consumer HIR; imported structures (e.g.
    // VonDiagnostic) live on imported_semantic_exports and must still provide
    // FieldGet layout ids — otherwise SMIR010 "no layout id" on error.message.
    // Skip empty / erased names. Always register distinct (namespace, name) shapes
    // so TextSpan{start,stop} is not erased by core.text TextSpan{offset,length}.
    for export in &module.imported_semantic_exports {
        for hir_struct in &export.structs {
            if hir_struct.fields.is_empty() || !hir_struct.generics.is_empty() {
                continue;
            }
            let simple = hir_struct.name.as_str();
            if matches!(simple, "any" | "null" | "object" | "Self" | "__auto" | "__opaque") {
                continue;
            }
            let storage = if hir_struct.is_value_type { MirStorageKind::Value } else { MirStorageKind::Reference };
            let layout = layout_for_struct(hir_struct, &mut plan, storage);
            register_declared_layout(&mut plan, hir_struct, layout);
        }
    }
    for singleton in &module.singletons {
        let hir_struct = crate::valkyrie::mir::singleton::singleton_as_struct(singleton);
        if !hir_struct.generics.is_empty() {
            continue;
        }
        let layout = layout_for_struct(&hir_struct, &mut plan, MirStorageKind::Reference);
        register_declared_layout(&mut plan, &hir_struct, layout);
    }
    plan
}

fn collect_module_structs<'a>(module: &'a HirModule, output: &mut Vec<&'a HirStruct>) {
    output.extend(module.structs.iter());
    for child in &module.submodules {
        collect_module_structs(child, output);
    }
}

pub fn ensure_layout_for_type(plan: &mut AggregateLayoutPlan, ty: &ValkyrieType) -> Option<LayoutId> {
    if let Some(id) = layout_id_for_type(ty, plan) {
        return Some(id);
    }
    let layout = match ty {
        ValkyrieType::Tuple(types) => layout_for_tuple(types, plan),
        ValkyrieType::FixedArray { element, length } => layout_for_fixed_array(element, *length, plan),
        _ => return None,
    };
    let id = layout.id;
    register_layout(plan, layout);
    Some(id)
}

/// Unite/Result/Option 运行时 `tag`/`payload` 载体布局，按 sum declaration ItemId 登记。
///
/// Matches CLR/Wasm GC convention: reference aggregate with `tag: i32` + opaque payload.
/// Payload uses utf8 as a stand-in ABI carrier (reference-sized); typed Fine/Fail
/// extraction goes through `SumPayloadGet`, not this field's declared type.
/// `display_name` 仅诊断/调试；禁止用名称合并或反查布局。
pub fn ensure_unite_tagged_layout(
    plan: &mut AggregateLayoutPlan,
    declaration: nyar_types::ItemId,
    display_name: &str,
) -> LayoutId {
    if let Some(id) = plan.declaration_to_layout.get(&declaration).copied() {
        if plan.layouts.iter().find(|layout| layout.id == id).is_some_and(|layout| {
            layout.fields.iter().any(|field| field.name == "tag") && layout.fields.iter().any(|field| field.name == "payload")
        }) {
            return id;
        }
    }
    let layout = build_aggregate_layout(
        next_layout_id(plan),
        display_name,
        String::new(),
        MirStorageKind::Reference,
        vec![
            FieldSpec { name: "tag".to_string(), ty: ValkyrieType::Integer32 { signed: true }, size: 4, align: 4 },
            FieldSpec { name: "payload".to_string(), ty: ValkyrieType::Utf8, size: 8, align: 8 },
        ],
    );
    let id = layout.id;
    plan.declaration_to_layout.insert(declaration, id);
    plan.layouts.push(layout);
    id
}

/// Ensure every unite in `sum_types` has a tagged aggregate layout for FieldGet contracts.
pub fn ensure_unite_layouts_for_sums(plan: &mut AggregateLayoutPlan, sum_types: &[super::MirSumDeclaration]) {
    for sum in sum_types {
        if sum.is_unite {
            let Some(declaration) = sum.declaration
            else {
                // 缺 declaration 不得按 sum.name 伪登记布局。
                continue;
            };
            ensure_unite_tagged_layout(plan, declaration, &sum.name);
        }
    }
}

pub fn layout_for_tuple(element_types: &[ValkyrieType], plan: &AggregateLayoutPlan) -> AggregateLayout {
    let name = layout_key_for_type(&ValkyrieType::Tuple(element_types.to_vec())).unwrap_or_else(|| "__tuple".to_string());
    build_aggregate_layout(next_layout_id(plan), &name, String::new(), MirStorageKind::Value, element_fields(element_types, plan))
}

pub fn layout_for_fixed_array(element_type: &ValkyrieType, length: usize, plan: &AggregateLayoutPlan) -> AggregateLayout {
    let name = layout_key_for_type(&ValkyrieType::FixedArray { element: Box::new(element_type.clone()), length })
        .unwrap_or_else(|| "__fixedarray".to_string());
    let fields = (0..length)
        .map(|index| {
            let (size, align) = scalar_layout(element_type, plan);
            FieldSpec { name: index.to_string(), ty: element_type.clone(), size, align }
        })
        .collect::<Vec<_>>();
    build_aggregate_layout(next_layout_id(plan), &name, String::new(), MirStorageKind::Value, fields)
}

fn layout_for_struct(hir_struct: &HirStruct, plan: &AggregateLayoutPlan, storage: MirStorageKind) -> AggregateLayout {
    let fields = hir_struct
        .fields
        .iter()
        .map(|field| {
            let (size, align) = scalar_layout(&field.ty, plan);
            FieldSpec { name: field.name.to_string(), ty: field.ty.clone(), size, align }
        })
        .collect();
    let namespace = hir_struct.namespace.iter().map(|id| id.as_str()).collect::<Vec<_>>().join(".");
    build_aggregate_layout(next_layout_id(plan), &hir_struct.name.to_string(), namespace, storage, fields)
}

fn layout_qualified_key(layout: &AggregateLayout) -> String {
    if layout.namespace.is_empty() { layout.name.clone() } else { format!("{}.{}", layout.namespace, layout.name) }
}

fn register_declared_layout(plan: &mut AggregateLayoutPlan, declaration: &HirStruct, layout: AggregateLayout) {
    let Some(item) = declaration.constructor_declaration
    else {
        // 缺 declaration 不得按 struct 名伪登记布局旁路。
        return;
    };
    if plan.declaration_to_layout.contains_key(&item) {
        return;
    }
    plan.declaration_to_layout.insert(item, layout.id);
    if layout.storage == MirStorageKind::Value {
        // value_type_names 仍供诊断/过渡存储分类；查询生产布局只认 declaration_to_layout。
        plan.value_type_names.insert(layout_qualified_key(&layout));
    }
    // 禁止把名义结构体写入 type_name_to_layout；该表仅服务合成 tuple/fixedarray 键。
    plan.layouts.push(layout);
}

/// 仅登记合成 tuple / fixedarray 布局；键为 `layout_key_for_type` 产物。
fn register_layout(plan: &mut AggregateLayoutPlan, layout: AggregateLayout) {
    let qualified = layout_qualified_key(&layout);
    if plan.type_name_to_layout.contains_key(&qualified) {
        return;
    }
    if layout.storage == MirStorageKind::Value {
        plan.value_type_names.insert(qualified.clone());
    }
    plan.type_name_to_layout.insert(qualified, layout.id);
    plan.layouts.push(layout);
}

fn next_layout_id(plan: &AggregateLayoutPlan) -> LayoutId {
    plan.layouts.iter().map(|layout| layout.id).max().unwrap_or(0) + 1
}

struct FieldSpec {
    name: String,
    ty: ValkyrieType,
    size: u32,
    align: u32,
}

fn element_fields(element_types: &[ValkyrieType], plan: &AggregateLayoutPlan) -> Vec<FieldSpec> {
    element_types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let (size, align) = scalar_layout(ty, plan);
            FieldSpec { name: index.to_string(), ty: ty.clone(), size, align }
        })
        .collect()
}

fn build_aggregate_layout(id: LayoutId, name: &str, namespace: String, storage: MirStorageKind, specs: Vec<FieldSpec>) -> AggregateLayout {
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::with_capacity(specs.len());
    for spec in specs {
        offset = align_offset(offset, spec.align);
        // 布局字段类型必须严格 concretize；禁止 lossy 把 Self/Generic/Nominal 擦成伪 Named。
        let ty = concretize_type(&spec.ty).unwrap_or_else(|err| {
            panic!(
                "aggregate layout `{namespace}::{name}` field `{}` 无法严格 concretize: {err}",
                spec.name
            )
        });
        fields.push(FieldLayout { name: spec.name, ty, offset, size: spec.size, align: spec.align });
        offset += spec.size;
        max_align = max_align.max(spec.align);
    }
    // Empty aggregates would otherwise get size=0 and fail SMIR010; language-level
    // empty structs still need a non-zero carrier for layout contracts.
    let size = align_offset(offset, max_align).max(if fields.is_empty() { max_align.max(1) } else { 0 });
    AggregateLayout { id, name: name.to_string(), namespace, storage, size, align: max_align.max(1), fields }
}

fn scalar_layout(ty: &ValkyrieType, plan: &AggregateLayoutPlan) -> (u32, u32) {
    match ty {
        ValkyrieType::Boolean | ValkyrieType::Character => (4, 4),
        ValkyrieType::Integer8 { .. } | ValkyrieType::Integer16 { .. } | ValkyrieType::Integer32 { .. } | ValkyrieType::Float32 => (4, 4),
        ValkyrieType::Integer64 { .. } | ValkyrieType::Float64 => (8, 8),
        ValkyrieType::Nominal { declaration, .. } => plan
            .declaration_to_layout
            .get(declaration)
            .and_then(|id| plan.layouts.iter().find(|layout| layout.id == *id))
            .map(|layout| (layout.size, layout.align))
            .unwrap_or((8, 8)),
        ValkyrieType::Tuple(types) => inline_aggregate_size(&ValkyrieType::Tuple(types.clone()), plan),
        ValkyrieType::FixedArray { element, length } => {
            inline_aggregate_size(&ValkyrieType::FixedArray { element: element.clone(), length: *length }, plan)
        }
        _ => (8, 8),
    }
}

fn inline_aggregate_size(ty: &ValkyrieType, plan: &AggregateLayoutPlan) -> (u32, u32) {
    layout_id_for_type(ty, plan)
        .and_then(|id| plan.layouts.iter().find(|layout| layout.id == id))
        .map(|layout| (layout.size, layout.align))
        .unwrap_or_else(|| {
            let fields = match ty {
                ValkyrieType::Tuple(types) => element_fields(types, plan),
                ValkyrieType::FixedArray { element, length } => (0..*length)
                    .map(|index| {
                        let (size, align) = scalar_layout(element, plan);
                        FieldSpec { name: index.to_string(), ty: element.as_ref().clone(), size, align }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let layout = build_aggregate_layout(0, "__tmp", String::new(), MirStorageKind::Value, fields);
            (layout.size, layout.align)
        })
}

fn align_offset(offset: u32, align: u32) -> u32 {
    if align <= 1 {
        return offset;
    }
    ((offset + align - 1) / align) * align
}

fn type_layout_key_component(ty: &ValkyrieType) -> String {
    match ty {
        ValkyrieType::Named(name) => name.to_string(),
        ValkyrieType::Boolean => "bool".to_string(),
        ValkyrieType::Character => "char".to_string(),
        ValkyrieType::Integer8 { signed } => format!("i8_{signed}"),
        ValkyrieType::Integer16 { signed } => format!("i16_{signed}"),
        ValkyrieType::Integer32 { signed } => format!("i32_{signed}"),
        ValkyrieType::Integer64 { signed } => format!("i64_{signed}"),
        ValkyrieType::Integer128 { signed } => format!("i128_{signed}"),
        ValkyrieType::Float32 => "f32".to_string(),
        ValkyrieType::Float64 => "f64".to_string(),
        ValkyrieType::Utf8 => "utf8".to_string(),
        ValkyrieType::Utf16 => "utf16".to_string(),
        ValkyrieType::Tuple(types) => format!("tuple_{}", types.iter().map(type_layout_key_component).collect::<Vec<_>>().join("_")),
        ValkyrieType::FixedArray { element, length } => {
            format!("arr{length}_{}", type_layout_key_component(element))
        }
        ValkyrieType::Array(inner) => format!("heaparr_{}", type_layout_key_component(inner)),
        _ => "ref".to_string(),
    }
}

#[cfg(test)]
mod strict_layout_tests {
    use super::*;
    use nyar::NyarType;

    #[test]
    fn unite_tagged_layout_uses_strict_concretize() {
        let mut plan = AggregateLayoutPlan::default();
        let declaration = nyar_types::ItemId::from_index(7).expect("test ItemId");
        let id = ensure_unite_tagged_layout(&mut plan, declaration, "Option");
        assert_eq!(plan.declaration_to_layout.get(&declaration).copied(), Some(id));
        assert!(!plan.type_name_to_layout.contains_key("Option"), "unite 布局不得按短名键合并");
        let layout = plan.layouts.iter().find(|layout| layout.id == id).expect("layout");
        assert_eq!(layout.fields[0].ty, NyarType::Integer32 { signed: true });
        assert_eq!(layout.fields[1].ty, NyarType::Utf8);
    }

    #[test]
    #[should_panic(expected = "无法严格 concretize")]
    fn layout_panics_on_unsubstituted_self_field() {
        let _ = build_aggregate_layout(
            1,
            "Bad",
            String::new(),
            MirStorageKind::Value,
            vec![FieldSpec { name: "x".to_string(), ty: ValkyrieType::SelfType, size: 8, align: 8 }],
        );
    }
}
