//! Value vs reference storage classification for MIR aggregates.
//!
//! `structure`, tuple, and fixed array must stay **value** (`MirStorageKind::Value`):
//! inline layout, by-value move/copy, stack or linear-memory slots — never GC heap objects.
//! `class` and heap `[T]` remain **reference** (`MirStorageKind::Reference`).
//!
//! Layout payload types live in `nyar_types`; this module owns Valkyrie HIR → layout planning.

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::StorageKind;

use crate::compiler::{
    emitter::frontend_contract::concretize_type,
    types::hir::{HirModule, HirStruct, ValkyrieType},
};

pub use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldLayout, FlagsLayout, LayoutId, LayoutKey, SumTypeLayout, SumVariantLayout};

/// Physical storage class for aggregate values at lowering time.
///
/// Type alias to the platform [`StorageKind`] so MIR and backends share one enum.
pub type MirStorageKind = StorageKind;

/// 判定类型的物理存储类别：值语义或引用语义。
///
/// 仅已冻结的 `Nominal` declaration 可判为值类型；短名不认亲。
pub fn storage_kind_for_type(ty: &ValkyrieType, value_type_declarations: &BTreeSet<nyar_types::ItemId>) -> MirStorageKind {
    match ty {
        ValkyrieType::Tuple(_) | ValkyrieType::FixedArray { .. } => MirStorageKind::Value,
        ValkyrieType::Array(_) => MirStorageKind::Reference,
        // 只认已冻结 Nominal declaration；禁止 Named 短名认亲 value 存储。
        ValkyrieType::Nominal { declaration, .. } if value_type_declarations.contains(declaration) => MirStorageKind::Value,
        _ => MirStorageKind::Reference,
    }
}

/// 为合成 tuple / fixed array 类型生成结构化布局键；名义类型返回 `None`。
///
/// 键只由平台类型身份构成；元素类型无法严格 concretize 时不产生键，
/// 由后续布局构造失败关闭，禁止回落到名称。
fn synthetic_layout_key(ty: &ValkyrieType) -> Option<LayoutKey> {
    match ty {
        ValkyrieType::Tuple(types) => {
            let elements = types.iter().map(concretize_type).collect::<Result<Vec<_>, _>>().ok()?;
            Some(LayoutKey::Tuple(elements))
        }
        ValkyrieType::FixedArray { element, length } => {
            Some(LayoutKey::FixedArray { element: Box::new(concretize_type(element).ok()?), length: *length })
        }
        _ => None,
    }
}

/// 查询类型在计划中登记的布局 id：优先名义 declaration，其次合成键。
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
    // 合成 tuple / fixedarray 结构键；禁止 Future/Promise 等短名透明解包。
    let key = synthetic_layout_key(ty)?;
    plan.synthetic_layouts.get(&key).copied()
}

/// 从 HIR 收集聚合布局计划：结构体、导入结构体与 singleton 载体。
pub fn compute_aggregate_layout_plan(module: &HirModule) -> AggregateLayoutPlan {
    let mut plan = AggregateLayoutPlan::default();
    let mut structs = Vec::new();
    collect_module_structs(module, &mut structs);
    // 纯类型级标记字段（零字段值类型 + 泛型实参）不进入运行时布局，与 MIR 字段事实同口径。
    let markers = type_level_marker_declarations(module);
    // 核心 primitive（如 `structure i32 { }`）的声明身份在 HIR 保留为 Nominal{ItemId}，
    // 但字段布局必须使用其平台载体形状。
    let mut primitive_carriers = BTreeMap::new();
    for hir_struct in &structs {
        if !hir_struct.is_value_type || !hir_struct.generics.is_empty() || !hir_struct.fields.is_empty() {
            continue;
        }
        let Some(declaration) = hir_struct.constructor_declaration
        else {
            continue;
        };
        let Some(carrier) = crate::compiler::hir::type_lowering::numeric_builtin_cast_type(hir_struct.name.as_str())
        else {
            continue;
        };
        primitive_carriers.insert(declaration, carrier);
    }
    for hir_struct in structs {
        // 开放泛型声明不得生成平台布局（字段仍是 binder）；与 SumTypeLayout 同口径。
        if !hir_struct.generics.is_empty() {
            continue;
        }
        let storage = if hir_struct.is_value_type { MirStorageKind::Value } else { MirStorageKind::Reference };
        let layout = layout_for_struct(hir_struct, &mut plan, storage, &primitive_carriers, &markers);
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
            let layout = layout_for_struct(hir_struct, &mut plan, storage, &primitive_carriers, &markers);
            register_declared_layout(&mut plan, hir_struct, layout);
        }
    }
    for singleton in &module.singletons {
        let hir_struct = crate::compiler::mir::singleton::singleton_as_struct(singleton);
        if !hir_struct.generics.is_empty() {
            continue;
        }
        let layout = layout_for_struct(&hir_struct, &mut plan, MirStorageKind::Reference, &primitive_carriers, &markers);
        register_declared_layout(&mut plan, &hir_struct, layout);
    }
    plan
}

/// 具体泛型实例的聚合布局结果：布局身份 + 与声明同序的代入后字段类型。
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceLayout {
    /// 新登记的实例布局身份。
    pub id: LayoutId,
    /// 与 `MirStruct::fields` 同序的字段类型（已完成 binder 代入）。
    pub field_types: Vec<ValkyrieType>,
}

/// 按实例实参代入声明字段，返回与 `MirStruct::fields` 同序的字段类型。
///
/// 只认已冻结的 declaration 与 binder：形参数目不符、缺 binder、或实参仍含
/// 开放泛型形参时返回 `None`；禁止按字段名或短名猜代入结果。
pub fn instantiate_fields(
    structure: &crate::compiler::mir::MirStruct,
    instance: &ValkyrieType,
) -> Option<Vec<ValkyrieType>> {
    let ValkyrieType::Nominal { declaration, arguments } = instance
    else {
        return None;
    };
    if structure.declaration != Some(*declaration) || arguments.len() != structure.generics.len() {
        return None;
    }
    let substitutions = structure
        .generics
        .iter()
        .zip(arguments)
        .map(|(generic, argument)| Some((generic.binder?, argument.clone())))
        .collect::<Option<BTreeMap<_, _>>>()?;
    if substitutions.len() != structure.generics.len() {
        return None;
    }
    Some(
        structure
            .fields
            .iter()
            .map(|field| {
                let ty = crate::compiler::hir::overload::substitute_type_vars(&field.ty, &substitutions);
                super::struct_facts::resolve_self_type_with_owner(&ty, Some(instance))
            })
            .collect(),
    )
}

/// 为具体泛型实例构造聚合布局并登记进计划。
///
/// 字段类型严格由已冻结声明 spelling + 实参生成平台形状，与 emitter `lower_type`
/// 同口径；缺拼写、实参仍含开放形参、或声明与实例不匹配时返回 `None`（失败关闭）。
/// 开放泛型声明本身仍由 [`compute_aggregate_layout_plan`] 跳过，不在此合成布局。
pub fn instance_layout_for_type(
    plan: &mut AggregateLayoutPlan,
    structure: &crate::compiler::mir::MirStruct,
    instance: &ValkyrieType,
    declaration_spellings: &BTreeMap<nyar_types::ItemId, String>,
) -> Option<InstanceLayout> {
    if type_contains_open_generic(instance) {
        return None;
    }
    let field_types = instantiate_fields(structure, instance)?;
    let storage = if structure.is_value_type { MirStorageKind::Value } else { MirStorageKind::Reference };
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::with_capacity(field_types.len());
    for (declared, ty) in structure.fields.iter().zip(field_types.iter()) {
        let layout_ty = layout_field_type(ty, declaration_spellings)?;
        let (size, align) = layout_field_extent(&layout_ty);
        offset = align_offset(offset, align);
        fields.push(FieldLayout { name: declared.name.clone(), ty: layout_ty, offset, size, align });
        offset += size;
        max_align = max_align.max(align);
    }
    let size = align_offset(offset, max_align).max(if fields.is_empty() { max_align.max(1) } else { 0 });
    let id = next_layout_id(plan);
    plan.layouts.push(AggregateLayout {
        id,
        name: structure.name.clone(),
        namespace: structure.namespace.clone(),
        key: None,
        storage,
        size,
        align: max_align.max(1),
        fields,
    });
    Some(InstanceLayout { id, field_types })
}

/// 类型是否仍含开放泛型形参；这类实例只有语义身份，没有单一物理布局。
fn type_contains_open_generic(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Generic(_) => true,
        ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(type_contains_open_generic),
        ValkyrieType::Apply(base, arguments) => type_contains_open_generic(base) || arguments.iter().any(type_contains_open_generic),
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => type_contains_open_generic(element),
        ValkyrieType::FixedArray { element, .. } => type_contains_open_generic(element),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
            items.iter().any(type_contains_open_generic)
        }
        ValkyrieType::Function(function) => {
            function.params.iter().any(type_contains_open_generic) || type_contains_open_generic(&function.return_type)
        }
        _ => false,
    }
}

/// 字段的平台布局类型：结构化形状走 [`concretize_type`]，名义形状按已冻结
/// 声明 spelling 生成 `Named`/primitive，非空实参再包一层 `Apply`。
///
/// 与 emitter `lower_type` 同口径（声明 spelling 先查 primitive 别名，否则
/// `Named`；实参递归）。任一环节缺事实返回 `None`，禁止按短名猜载体。
fn layout_field_type(
    ty: &ValkyrieType,
    declaration_spellings: &BTreeMap<nyar_types::ItemId, String>,
) -> Option<nyar_types::NyarType> {
    if let Ok(concrete) = concretize_type(ty) {
        return Some(concrete);
    }
    match ty {
        ValkyrieType::Nominal { declaration, arguments } => {
            let spelling = declaration_spellings.get(declaration)?;
            let base = nyar_types::NyarType::from_surface_primitive(spelling)
                .unwrap_or_else(|| nyar_types::NyarType::Named(nyar_types::Identifier::new(spelling)));
            if arguments.is_empty() {
                return Some(base);
            }
            Some(nyar_types::NyarType::Apply(
                Box::new(base),
                arguments.iter().map(|argument| layout_field_type(argument, declaration_spellings)).collect::<Option<Vec<_>>>()?,
            ))
        }
        ValkyrieType::Apply(base, arguments) => Some(nyar_types::NyarType::Apply(
            Box::new(layout_field_type(base, declaration_spellings)?),
            arguments.iter().map(|argument| layout_field_type(argument, declaration_spellings)).collect::<Option<Vec<_>>>()?,
        )),
        ValkyrieType::Array(element) => Some(nyar_types::NyarType::Array(Box::new(layout_field_type(element, declaration_spellings)?))),
        ValkyrieType::FixedArray { element, length } => Some(nyar_types::NyarType::FixedArray {
            element: Box::new(layout_field_type(element, declaration_spellings)?),
            length: *length,
        }),
        ValkyrieType::Tuple(items) => Some(nyar_types::NyarType::Tuple(
            items.iter().map(|item| layout_field_type(item, declaration_spellings)).collect::<Option<Vec<_>>>()?,
        )),
        ValkyrieType::Nullable(element) => {
            Some(nyar_types::NyarType::Nullable(Box::new(layout_field_type(element, declaration_spellings)?)))
        }
        ValkyrieType::Union(items) => Some(nyar_types::NyarType::Union(
            items.iter().map(|item| layout_field_type(item, declaration_spellings)).collect::<Option<Vec<_>>>()?,
        )),
        _ => None,
    }
}

/// 字段字节大小与对齐：标量按宽度，其余按引用槽。
fn layout_field_extent(ty: &nyar_types::NyarType) -> (u32, u32) {
    match ty {
        nyar_types::NyarType::Integer64 { .. }
        | nyar_types::NyarType::Integer128 { .. }
        | nyar_types::NyarType::Float64 => (8, 8),
        nyar_types::NyarType::Bottom
        | nyar_types::NyarType::Unit
        | nyar_types::NyarType::Boolean
        | nyar_types::NyarType::Integer8 { .. }
        | nyar_types::NyarType::Integer16 { .. }
        | nyar_types::NyarType::Integer32 { .. }
        | nyar_types::NyarType::Float32
        | nyar_types::NyarType::Character
        | nyar_types::NyarType::Utf8
        | nyar_types::NyarType::Utf16 => (4, 4),
        _ => (8, 8),
    }
}

fn collect_module_structs<'a>(module: &'a HirModule, output: &mut Vec<&'a HirStruct>) {
    output.extend(module.structs.iter());
    for child in &module.submodules {
        collect_module_structs(child, output);
    }
}

/// 收集「纯类型级标记」声明：值类型（`structure`）、零字段、带泛型形参。
///
/// 这类声明没有任何运行时字段，只能通过名义类型实参参与类型身份
/// （`std.collection.Phantom<T>` 即此形状）。判定只取声明结构事实，不含名称、
/// 命名空间或字段名；引用类型（`class`）与无泛型空结构不在内。
pub fn type_level_marker_declarations(module: &HirModule) -> BTreeSet<nyar_types::ItemId> {
    fn collect(module: &HirModule, output: &mut BTreeSet<nyar_types::ItemId>) {
        for structure in module.structs.iter().chain(module.imported_semantic_exports.iter().flat_map(|export| export.structs.iter())) {
            if !structure.is_value_type || !structure.fields.is_empty() || structure.generics.is_empty() {
                continue;
            }
            if let Some(declaration) = structure.constructor_declaration {
                output.insert(declaration);
            }
        }
        for child in &module.submodules {
            collect(child, output);
        }
    }
    let mut markers = BTreeSet::new();
    collect(module, &mut markers);
    markers
}

/// 字段声明类型是纯类型级标记时，该字段只绑定类型实参，不承载运行时布局事实。
///
/// 只认已冻结的 [`ValkyrieType::Nominal`] declaration 身份；`Named`/限定名字符串
/// 不参与认亲，避免按名字把普通字段误判为标记。
pub fn is_type_level_marker_field(ty: &ValkyrieType, markers: &BTreeSet<nyar_types::ItemId>) -> bool {
    match ty {
        ValkyrieType::Nominal { declaration, .. } => markers.contains(declaration),
        _ => false,
    }
}

/// 确保类型已在计划中有布局：缺失时为 tuple / fixed array 合成并登记新布局。
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
pub fn ensure_unite_tagged_layout(plan: &mut AggregateLayoutPlan, declaration: nyar_types::ItemId, display_name: &str) -> LayoutId {
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
    let mut layout =
        build_aggregate_layout(next_layout_id(plan), "tuple", String::new(), MirStorageKind::Value, element_fields(element_types, plan));
    layout.key = synthetic_layout_key(&ValkyrieType::Tuple(element_types.to_vec()));
    layout
}

pub fn layout_for_fixed_array(element_type: &ValkyrieType, length: usize, plan: &AggregateLayoutPlan) -> AggregateLayout {
    let fields = (0..length)
        .map(|index| {
            let (size, align) = scalar_layout(element_type, plan);
            FieldSpec { name: index.to_string(), ty: element_type.clone(), size, align }
        })
        .collect::<Vec<_>>();
    let mut layout = build_aggregate_layout(next_layout_id(plan), "fixedarray", String::new(), MirStorageKind::Value, fields);
    layout.key = synthetic_layout_key(&ValkyrieType::FixedArray { element: Box::new(element_type.clone()), length });
    layout
}

fn layout_for_struct(
    hir_struct: &HirStruct,
    plan: &AggregateLayoutPlan,
    storage: MirStorageKind,
    primitive_carriers: &BTreeMap<nyar_types::ItemId, ValkyrieType>,
    markers: &BTreeSet<nyar_types::ItemId>,
) -> AggregateLayout {
    let fields = hir_struct
        .fields
        .iter()
        .filter(|field| !is_type_level_marker_field(&field.ty, markers))
        .map(|field| {
            // 核心 primitive 声明在 HIR 中冻结为 Nominal{ItemId}；布局必须回到平台载体，
            // 否则字段布局缺少平台 NyarType::Nominal 形状。
            let ty = field
                .ty
                .nominal_declaration()
                .and_then(|declaration| primitive_carriers.get(&declaration))
                .cloned()
                .unwrap_or_else(|| field.ty.clone());
            let (size, align) = scalar_layout(&ty, plan);
            FieldSpec { name: field.name.to_string(), ty, size, align }
        })
        .collect();
    let namespace = hir_struct.namespace.iter().map(|id| id.as_str()).collect::<Vec<_>>().join(".");
    build_aggregate_layout(next_layout_id(plan), &hir_struct.name.to_string(), namespace, storage, fields)
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
        plan.value_type_declarations.insert(item);
    }
    // 名义结构体不得写入合成布局表；后端须读 layout.storage / value_type_declarations。
    plan.layouts.push(layout);
}

/// 仅登记合成 tuple / fixedarray 布局；键为结构化 [`LayoutKey`]。
fn register_layout(plan: &mut AggregateLayoutPlan, layout: AggregateLayout) {
    let Some(key) = layout.key.clone()
    else {
        // 无结构键的布局不能进入合成表。
        return;
    };
    if plan.synthetic_layouts.contains_key(&key) {
        return;
    }
    plan.synthetic_layouts.insert(key, layout.id);
    plan.layouts.push(layout);
}

fn next_layout_id(plan: &AggregateLayoutPlan) -> LayoutId {
    plan.layouts.iter().map(|layout| layout.id).max().unwrap_or(0) + 1
}

/// 聚合布局字段的规格描述：未 concretize 前的字段名 / 类型 / 字节大小 / 字节对齐。
#[doc(hidden)]
pub struct FieldSpec {
    /// 字段名；tuple / fixed array 用序号字符串。
    pub name: String,
    /// 未 concretize 的字段类型。
    pub ty: ValkyrieType,
    /// 字段字节大小。
    pub size: u32,
    /// 字段字节对齐。
    pub align: u32,
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

/// 按字段规格计算偏移与对齐并构造聚合布局；字段类型必须严格 concretize。
#[doc(hidden)]
pub fn build_aggregate_layout(id: LayoutId, name: &str, namespace: String, storage: MirStorageKind, specs: Vec<FieldSpec>) -> AggregateLayout {
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::with_capacity(specs.len());
    for spec in specs {
        offset = align_offset(offset, spec.align);
        // 布局字段类型必须严格 concretize；禁止 lossy 把 Self/Generic/Nominal 擦成伪 Named。
        let ty = concretize_type(&spec.ty)
            .unwrap_or_else(|err| panic!("aggregate layout `{namespace}::{name}` field `{}` 无法严格 concretize: {err}", spec.name));
        fields.push(FieldLayout { name: spec.name, ty, offset, size: spec.size, align: spec.align });
        offset += spec.size;
        max_align = max_align.max(spec.align);
    }
    // Empty aggregates would otherwise get size=0 and fail SMIR010; language-level
    // empty structs still need a non-zero carrier for layout contracts.
    let size = align_offset(offset, max_align).max(if fields.is_empty() { max_align.max(1) } else { 0 });
    AggregateLayout { id, name: name.to_string(), namespace, key: None, storage, size, align: max_align.max(1), fields }
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
