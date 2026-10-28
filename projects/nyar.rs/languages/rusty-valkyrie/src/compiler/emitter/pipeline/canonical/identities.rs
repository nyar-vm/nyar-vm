use std::collections::BTreeMap;

use nyar_types::{
    CanonicalPrimitiveType, CanonicalTypeKind, FieldId, FieldRecord, Identifier, LinkedSemanticProgram, NominalInstanceId,
    NominalInstanceRecord, NominalValueSemantics, StructuredDiagnosticSet, TypeId,
};

use crate::compiler::{
    emitter::frontend_contract::concretize_type,
    mir::{instance_layout_for_type, MirModule},
    types::hir::ValkyrieType,
};

use super::errors::{error, error_without_module, type_id};

pub(crate) type AggregateIdentity = (TypeId, NominalValueSemantics);

/// 聚合声明登记结果：声明 nominal、声明字段，以及 declaration TypeId → 声明 nominal 的映射。
pub(crate) struct AggregateDeclarations {
    pub nominals: BTreeMap<NominalInstanceId, AggregateIdentity>,
    pub field_records: BTreeMap<FieldId, FieldRecord>,
    /// 声明 TypeId → 声明 `NominalInstanceId`；禁止按名称反查 owner。
    pub declaration_nominals: BTreeMap<TypeId, NominalInstanceId>,
}

pub(crate) fn collect_aggregate_identities(
    module: &MirModule,
    types: &BTreeMap<ValkyrieType, TypeId>,
) -> Result<AggregateDeclarations, StructuredDiagnosticSet> {
    let mut nominals = BTreeMap::new();
    let mut field_records = BTreeMap::new();
    let mut declaration_nominals = BTreeMap::new();
    for aggregate in &module.structs {
        let qualified =
            if aggregate.namespace.is_empty() { aggregate.name.clone() } else { format!("{}.{}", aggregate.namespace, aggregate.name) };
        let item =
            aggregate.declaration.ok_or_else(|| error_without_module("CAN018", format!("聚合 `{qualified}` 缺少 declaration ItemId")))?;
        let ty = ValkyrieType::Nominal { declaration: item, arguments: Vec::new() };
        let declaration = types.get(&ty).copied().ok_or_else(|| error_without_module("CAN018", format!("聚合 `{qualified}` 缺少类型事实")))?;
        let nominal = aggregate.nominal;
        // 值语义事实由上游 `structure`/`class` 关键字冻结：`structure` 是值类型，`class` 是引用类型。
        let semantics = if aggregate.is_value_type { NominalValueSemantics::Value } else { NominalValueSemantics::Reference };
        if nominals.insert(nominal, (declaration, semantics)).is_some() {
            return Err(error_without_module("CAN020", format!("聚合 identity 重复: {qualified}")));
        }
        if declaration_nominals.insert(declaration, nominal).is_some() {
            return Err(error_without_module("CAN020", format!("聚合声明 identity 重复: {qualified}")));
        }
        for field in &aggregate.fields {
            if field_records.insert(field.id, FieldRecord { owner: nominal, ty: type_id(types, &field.ty)? }).is_some() {
                return Err(error_without_module("CAN022", format!("字段 identity 重复: {qualified}.{}", field.name)));
            }
        }
    }
    for sum in &module.sum_types {
        let item = sum.declaration.ok_or_else(|| error_without_module("CAN018", format!("sum `{}` 缺少 declaration ItemId", sum.name)))?;
        let ty = ValkyrieType::Nominal { declaration: item, arguments: Vec::new() };
        let declaration = types.get(&ty).copied().ok_or_else(|| error_without_module("CAN018", format!("sum `{}` 缺少类型事实", sum.name)))?;
        // 值语义事实由前端声明冻结：enum/unite 默认就是值类型（Value），与 structure 相同。
        let semantics = if sum.is_value_type { NominalValueSemantics::Value } else { NominalValueSemantics::Reference };
        if nominals.insert(sum.nominal, (declaration, semantics)).is_some() {
            return Err(error_without_module("CAN020", format!("sum identity 重复: {}", sum.name)));
        }
        declaration_nominals.insert(declaration, sum.nominal);
    }
    Ok(AggregateDeclarations { nominals, field_records, declaration_nominals })
}

/// 具体泛型聚合实例的登记结果。
#[derive(Debug, Default)]
pub(crate) struct AggregateInstanceBindings {
    /// 实例类型 TypeId（含声明 TypeId）→ nominal 实例。
    pub by_type: BTreeMap<TypeId, NominalInstanceId>,
    /// (nominal 实例, 声明字段) → 实例字段；声明侧保留恒等映射。
    pub field_by_owner: BTreeMap<(NominalInstanceId, FieldId), FieldId>,
}

/// 为具体泛型聚合实例登记实例级布局与实例级字段身份。
///
/// 声明级布局只服务单态声明；泛型实例的字段类型必须先按 binder 代入，
/// 再冻结为独立 `FieldId` 与 `FieldRecord`。任一实参仍含开放形参、
/// 或代入后的字段类型没有类型事实时失败关闭，禁止复用声明布局。
pub(crate) fn register_aggregate_nominal_instances(
    module: &MirModule,
    type_values: &BTreeMap<ValkyrieType, TypeId>,
    declaration_nominals: &BTreeMap<TypeId, NominalInstanceId>,
    item_spellings: &BTreeMap<nyar_types::ItemId, String>,
    linked: &mut LinkedSemanticProgram,
    nominals: &mut BTreeMap<NominalInstanceId, AggregateIdentity>,
    field_records: &mut BTreeMap<FieldId, FieldRecord>,
) -> Result<AggregateInstanceBindings, StructuredDiagnosticSet> {
    let mut bindings = AggregateInstanceBindings::default();
    let mut next_nominal = nominals.keys().map(|id| id.index()).max().map(|index| index + 1).unwrap_or(0);
    // 实例字段身份必须在全部声明字段之后稠密分配，避免与 HIR 已有身份冲突。
    let mut next_field = field_records
        .keys()
        .map(|field| field.index())
        .chain(module.structs.iter().flat_map(|structure| structure.fields.iter().map(|field| field.id.index())))
        .chain(module.sum_types.iter().flat_map(|sum| sum.variants.iter().flat_map(|variant| variant.fields.iter().map(|field| field.id.index()))))
        .max()
        .map(|index| index + 1)
        .unwrap_or(0);
    for structure in &module.structs {
        let Some(declaration) = structure.declaration
        else {
            continue;
        };
        let declared = ValkyrieType::Nominal { declaration, arguments: Vec::new() };
        let declaration_type = type_id(type_values, &declared)?;
        let Some(nominal) = declaration_nominals.get(&declaration_type).copied()
        else {
            return Err(error_without_module(
                "CAN058",
                format!("聚合 `{}` 缺少声明级 nominal 身份", structure.qualified_name()),
            ));
        };
        // 声明侧字段身份恒等保留：单态聚合与未实例化路径继续使用声明字段。
        for field in &structure.fields {
            bindings.field_by_owner.insert((nominal, field.id), field.id);
        }
        bindings.by_type.insert(declaration_type, nominal);
        if structure.generics.is_empty() {
            continue;
        }
        let Some(semantics) = nominals.get(&nominal).map(|(_, semantics)| *semantics)
        else {
            return Err(error_without_module(
                "CAN056",
                format!("聚合 `{}` 缺少冻结的 NominalValueSemantics", structure.qualified_name()),
            ));
        };
        for (concrete, concrete_type) in type_values {
            let ValkyrieType::Nominal { declaration: instance_declaration, arguments } = concrete
            else {
                continue;
            };
            if *instance_declaration != declaration || arguments.is_empty() || contains_open_generic(concrete) {
                continue;
            }
            // 已登记过的实例不重复铸造身份。
            if bindings.by_type.contains_key(concrete_type) {
                continue;
            }
            let Some(layout) = instance_layout_for_type(&mut linked.aggregate_layouts, structure, concrete, item_spellings)
            else {
                return Err(error_without_module(
                    "CAN058",
                    format!("聚合实例 `{}` 无法按已冻结声明生成实例级布局", structure.qualified_name()),
                ));
            };
            let instance = NominalInstanceId::from_index(next_nominal)
                .ok_or_else(|| error_without_module("CAN020", "聚合 instance identity 溢出"))?;
            next_nominal = next_nominal.checked_add(1).ok_or_else(|| error_without_module("CAN020", "聚合 instance identity 溢出"))?;
            nominals.insert(instance, (declaration_type, semantics));
            let mut instance_fields = Vec::with_capacity(structure.fields.len());
            for (index, (declared, field)) in structure.fields.iter().zip(layout.field_types.iter()).enumerate() {
                let id = FieldId::from_index(next_field).ok_or_else(|| error_without_module("CAN022", "实例字段 identity 溢出"))?;
                next_field = next_field.checked_add(1).ok_or_else(|| error_without_module("CAN022", "实例字段 identity 溢出"))?;
                let record = FieldRecord { owner: instance, ty: type_id(type_values, field)? };
                if field_records.insert(id, record).is_some() {
                    return Err(error_without_module(
                        "CAN022",
                        format!("实例字段 identity 冲突: {}.{}", structure.qualified_name(), declared.name),
                    ));
                }
                instance_fields.push(id);
                bindings.field_by_owner.insert((instance, declared.id), id);
                linked.aggregate_layout_by_field.insert(id, (layout.id, index as u32));
            }
            linked
                .nominal_instances
                .insert(instance, NominalInstanceRecord { declaration: declaration_type, ty: *concrete_type, semantics, fields: instance_fields });
            linked.aggregate_layout_by_type.insert(*concrete_type, layout.id);
            linked.aggregate_layout_by_nominal.insert(instance, layout.id);
            bindings.by_type.insert(*concrete_type, instance);
        }
    }
    Ok(bindings)
}

pub(crate) fn validate_callable_identities(module: &MirModule) -> Result<(), StructuredDiagnosticSet> {
    let mut instances = BTreeMap::new();
    for function in &module.functions {
        let Some(identity) = function.instance
        else {
            return Err(error(module, "CAN034", format!("函数 `{}` 缺少 Compiler callable identity", function.display_symbol)));
        };
        if instances.insert(identity, function.display_symbol.clone()).is_some() {
            return Err(error(module, "CAN001", format!("callable identity `{identity}` 重复")));
        }
    }
    for contract in &module.external_calls {
        let symbol = contract.symbol.to_string();
        let Some(identity) = contract.instance
        else {
            return Err(error(module, "CAN034", format!("导入 `{symbol}` 缺少 Compiler callable identity")));
        };
        if instances.insert(identity, symbol.clone()).is_some() {
            return Err(error(module, "CAN001", format!("callable identity `{identity}` 重复")));
        }
    }
    Ok(())
}

pub(crate) fn collect_types(module: &MirModule) -> Result<BTreeMap<ValkyrieType, TypeId>, StructuredDiagnosticSet> {
    if module.type_identities.is_empty() && (!module.functions.is_empty() || !module.external_calls.is_empty() || !module.structs.is_empty()) {
        return Err(error(module, "CAN035", "Semantic MIR 缺少 Compiler type identity 表"));
    }
    Ok(module.type_identities.clone())
}

/// 语言侧可作为数组下标 / `ArrayLength` 结果的类型：原生整数，或 `usize`/`isize` 等名义原语。
///
/// 登记进 `array_index_types` 后，Canonical 用 TypeId 集合认整数形状，不把 `Nominal{usize}` 改写成 Primitive。
pub(crate) fn is_array_index_valkyrie_type(ty: &ValkyrieType, module: &MirModule) -> bool {
    match ty {
        ValkyrieType::Integer8 { .. }
        | ValkyrieType::Integer16 { .. }
        | ValkyrieType::Integer32 { .. }
        | ValkyrieType::Integer64 { .. }
        | ValkyrieType::Integer128 { .. } => true,
        ValkyrieType::Named(name) => is_array_index_surface_name(name.as_str()),
        ValkyrieType::Nominal { declaration, arguments } if arguments.is_empty() => module
            .structs
            .iter()
            .find(|item| item.declaration == Some(*declaration))
            .is_some_and(|item| is_array_index_surface_name(item.name.as_str())),
        _ => false,
    }
}

fn is_array_index_surface_name(name: &str) -> bool {
    matches!(
        name,
        "usize"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "byte"
            | "sbyte"
            | "core.primitive.usize"
            | "core.primitive.isize"
            | "core::primitive::usize"
            | "core::primitive::isize"
    )
}

/// 把 primitive numeric 的 Named surface name 映射到 canonical primitive kind。
/// 返回 `None` 表示不是 primitive numeric，应继续走 CAN003 报错路径。
pub(crate) fn canonical_primitive_from_named(name: &str) -> Option<CanonicalPrimitiveType> {
    Some(match name {
        "boolean" | "Bool" | "bool" => CanonicalPrimitiveType::Bool,
        "i8" => CanonicalPrimitiveType::Integer { bits: 8, signed: true },
        "u8" | "byte" => CanonicalPrimitiveType::Integer { bits: 8, signed: false },
        "i16" => CanonicalPrimitiveType::Integer { bits: 16, signed: true },
        "u16" => CanonicalPrimitiveType::Integer { bits: 16, signed: false },
        "i32" | "int" | "Int" => CanonicalPrimitiveType::Integer { bits: 32, signed: true },
        "u32" | "uint" | "UInt" => CanonicalPrimitiveType::Integer { bits: 32, signed: false },
        "i64" | "long" | "Long" => CanonicalPrimitiveType::Integer { bits: 64, signed: true },
        "u64" | "ulong" => CanonicalPrimitiveType::Integer { bits: 64, signed: false },
        "i128" => CanonicalPrimitiveType::Integer { bits: 128, signed: true },
        "u128" => CanonicalPrimitiveType::Integer { bits: 128, signed: false },
        "f32" | "float32" => CanonicalPrimitiveType::Float { bits: 32 },
        "f64" | "float" | "Float" | "double" | "Double" => CanonicalPrimitiveType::Float { bits: 64 },
        _ => return None,
    })
}

pub(crate) fn canonical_type_kind(
    ty: &ValkyrieType,
    ids: &BTreeMap<ValkyrieType, TypeId>,
) -> Result<CanonicalTypeKind, StructuredDiagnosticSet> {
    let id = |value: &ValkyrieType| type_id(ids, value);
    Ok(match ty {
        ValkyrieType::Void => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Void),
        ValkyrieType::Unit => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Unit),
        ValkyrieType::Boolean => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Bool),
        ValkyrieType::Integer8 { signed } => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 8, signed: *signed }),
        ValkyrieType::Integer16 { signed } => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 16, signed: *signed }),
        ValkyrieType::Integer32 { signed } => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 32, signed: *signed }),
        ValkyrieType::Integer64 { signed } => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 64, signed: *signed }),
        ValkyrieType::Integer128 { signed } => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 128, signed: *signed }),
        ValkyrieType::Float32 => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Float { bits: 32 }),
        ValkyrieType::Float64 => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Float { bits: 64 }),
        ValkyrieType::Character => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Character),
        ValkyrieType::Utf8 => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Utf8),
        ValkyrieType::Utf16 => CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Utf16),
        ValkyrieType::Nominal { declaration: item, arguments } => {
            let declaration_ty = ValkyrieType::Nominal { declaration: *item, arguments: Vec::new() };
            let declaration = type_id(ids, &declaration_ty)?;
            CanonicalTypeKind::Nominal { declaration, arguments: arguments.iter().map(id).collect::<Result<_, _>>()? }
        }
        ValkyrieType::Named(name) => match canonical_primitive_from_named(name.as_str()) {
            Some(kind) => CanonicalTypeKind::Primitive(kind),
            None => {
                return Err(error_without_module(
                    "CAN003",
                    format!("未绑定的类型名 `{name}` 进入 canonical 类型表；必须在 HIR 绑定阶段冻结为 Nominal 或返回前端 TypeError"),
                ));
            }
        },
        // 停线清理：QualifiedPath 不得进入 canonical。若到达这里说明前端生成/绑定
        // 阶段未切断该路径，直接走通配错误（CAN003 "类型没有无损 canonical 形状"）。
        ValkyrieType::Apply(base, args) => {
            CanonicalTypeKind::Nominal { declaration: id(base)?, arguments: args.iter().map(id).collect::<Result<_, _>>()? }
        }
        ValkyrieType::Tuple(items) => CanonicalTypeKind::Tuple(items.iter().map(id).collect::<Result<_, _>>()?),
        ValkyrieType::Array(element) => CanonicalTypeKind::Array { element: id(element)?, length: None },
        ValkyrieType::FixedArray { element, length } => CanonicalTypeKind::Array { element: id(element)?, length: Some(*length as u64) },
        ValkyrieType::Nullable(element) => CanonicalTypeKind::Nullable(id(element)?),
        ValkyrieType::Union(items) => CanonicalTypeKind::Union(items.iter().map(id).collect::<Result<_, _>>()?),
        ValkyrieType::Intersection(items) => CanonicalTypeKind::Intersection(items.iter().map(id).collect::<Result<_, _>>()?),
        ValkyrieType::Function(function) => CanonicalTypeKind::Function {
            parameters: function.params.iter().map(id).collect::<Result<_, _>>()?,
            return_type: id(&function.return_type)?,
        },
        // 泛型形参：只有带 `binder` 的声明身份才是稳定类型事实；缺失 binder 的形参
        // 仍是未解析占位。形参不得按名字解析成任何实现。
        ValkyrieType::Generic(generic) => {
            let Some(binder) = generic.binder
            else {
                return Err(error_without_module(
                    "CAN003",
                    format!("泛型形参 `{}` 缺少 binder 身份，不得进入 canonical 类型表", generic.name),
                ));
            };
            CanonicalTypeKind::Generic { owner: binder.owner, ordinal: binder.ordinal }
        }
        _ => {
            return Err(error_without_module("CAN003", format!("类型没有无损 canonical 形状: {ty:?}")));
        }
    })
}

/// 为每个具体 `Nominal{ItemId, args}` 注册 sum 实例与已代入 payload；SumNew 按结果类型选取实例。
pub(crate) fn register_sum_nominal_instances(
    module: &MirModule,
    type_values: &BTreeMap<ValkyrieType, TypeId>,
    declaration_spellings: &BTreeMap<TypeId, Identifier>,
    linked: &mut LinkedSemanticProgram,
    nominals: &mut BTreeMap<NominalInstanceId, AggregateIdentity>,
) -> Result<BTreeMap<TypeId, NominalInstanceId>, StructuredDiagnosticSet> {
    let mut by_type = BTreeMap::new();
    let mut next_nominal = nominals.keys().map(|id| id.index()).max().map(|index| index + 1).unwrap_or(0);
    for sum in &module.sum_types {
        let item = sum.declaration.ok_or_else(|| error_without_module("CAN018", format!("sum `{}` 缺少 declaration ItemId", sum.name)))?;
        let declaration_ty = ValkyrieType::Nominal { declaration: item, arguments: Vec::new() };
        let declaration = type_id(type_values, &declaration_ty)?;
        // 值语义事实已由 collect_aggregate_identities 从 sum 声明登记进 nominals map。
        let existing_semantics = nominals
            .get(&sum.nominal)
            .map(|(_, semantics)| *semantics)
            .ok_or_else(|| error_without_module("CAN056", format!("sum `{}` 缺少冻结的 NominalValueSemantics", sum.name)))?;
        linked.nominal_instances.entry(sum.nominal).or_insert(NominalInstanceRecord {
            declaration,
            ty: declaration,
            semantics: existing_semantics,
            fields: Vec::new(),
        });
        // 开放/泛型声明侧不登记未代入 payload；具体实例下方补齐。
        for variant in &sum.variants {
            let payload_type =
                if sum.generics.is_empty() { variant.payload_type().map(|ty| type_id(type_values, &ty)).transpose()? } else { None };
            linked.variants.insert((sum.nominal, variant.id), nyar_types::VariantRecord { payload_type });
        }
        by_type.insert(declaration, sum.nominal);
    }
    for (ty, concrete_ty) in type_values {
        let ValkyrieType::Nominal { declaration: item, arguments } = ty
        else {
            continue;
        };
        if arguments.is_empty() || by_type.contains_key(concrete_ty) {
            continue;
        }
        let Some(sum) = module.sum_types.iter().find(|sum| sum.declaration == Some(*item))
        else {
            // 聚合（`structure`/`class`）实例的布局与字段身份由
            // `register_aggregate_nominal_instances` 单独登记；这里不复制第二份事实。
            continue;
        };
        let declaration = type_id(type_values, &ValkyrieType::Nominal { declaration: *item, arguments: Vec::new() })?;
        let nominal =
            NominalInstanceId::from_index(next_nominal).ok_or_else(|| error_without_module("CAN020", "sum instance identity 溢出"))?;
        next_nominal = next_nominal.checked_add(1).ok_or_else(|| error_without_module("CAN020", "sum instance identity 溢出"))?;
        // 值语义事实从 sum 声明继承；enum/unite 默认就是值类型。
        let semantics = if sum.is_value_type { NominalValueSemantics::Value } else { NominalValueSemantics::Reference };
        nominals.insert(nominal, (declaration, semantics));
        linked.nominal_instances.insert(nominal, NominalInstanceRecord { declaration, ty: *concrete_ty, semantics, fields: Vec::new() });
        // 仍带开放形参的实例只有语义身份，没有单一物理布局（与泛型聚合声明同口径）。
        let open_generic = arguments.iter().any(contains_open_generic);
        let mut layout_variants = Vec::with_capacity(sum.variants.len());
        for variant in &sum.variants {
            let instantiated = sum
                .instantiate_payload(variant, arguments)
                .ok_or_else(|| error_without_module("CAN018", format!("sum `{}` 实例化 payload 失败", sum.name)))?;
            let payload_type = instantiated.as_ref().map(|ty| type_id(type_values, ty)).transpose()?;
            linked.variants.insert((nominal, variant.id), nyar_types::VariantRecord { payload_type });
            if open_generic {
                continue;
            }
            // 布局侧仍写 NyarType；语义身份只认上方 VariantRecord 的 TypeId。
            // 名义 payload（`usize`、`SwissTableEntry<K, V>` 等）没有结构化目标形状，
            // 按声明身份已冻结的 surface spelling 表示，与 emitter `lower_type` 同口径。
            let layout_payload = instantiated
                .as_ref()
                .and_then(|ty| sum_payload_layout(ty, type_values, declaration_spellings));
            layout_variants.push(nyar_types::SumVariantLayout {
                id: variant.id,
                name: variant.name.clone(),
                tag: variant.tag,
                payload_type: layout_payload,
            });
        }
        if !open_generic {
            linked.sum_types.push(nyar_types::SumTypeLayout {
                nominal,
                name: sum.name.clone(),
                is_unite: sum.is_unite,
                tag_width: 4,
                variants: layout_variants,
            });
        }
        by_type.insert(*concrete_ty, nominal);
    }
    Ok(by_type)
}

/// sum variant payload 的布局侧形状。
///
/// 语义身份只认 `VariantRecord::payload_type`；这里只回答 RepresentationPlan 与物理
/// 布局需要的 `NyarType`。结构化 payload 走严格 `concretize_type`；名义 payload
/// （`usize`、`SwissTableEntry<K, V>` 等）没有平台 Nominal 形状，按声明 TypeId 已冻结的
/// surface spelling 表示，与 emitter `lower_type` 同口径，不按名字猜实现。
fn sum_payload_layout(
    ty: &ValkyrieType,
    type_values: &BTreeMap<ValkyrieType, TypeId>,
    declaration_spellings: &BTreeMap<TypeId, Identifier>,
) -> Option<nyar_types::NyarType> {
    if let Ok(concrete) = concretize_type(ty) {
        return Some(concrete);
    }
    let ValkyrieType::Nominal { declaration, arguments } = ty
    else {
        return None;
    };
    let declaration_id = type_values.get(&ValkyrieType::Nominal { declaration: *declaration, arguments: Vec::new() })?;
    let spelling = declaration_spellings.get(declaration_id)?;
    let layout = nyar_types::NyarType::from_surface_primitive(spelling.as_str())
        .unwrap_or_else(|| nyar_types::NyarType::Named(spelling.clone()));
    if arguments.is_empty() {
        return Some(layout);
    }
    Some(nyar_types::NyarType::Apply(
        Box::new(layout),
        arguments
            .iter()
            .map(|argument| sum_payload_layout(argument, type_values, declaration_spellings))
            .collect::<Option<Vec<_>>>()?,
    ))
}

/// 类型是否仍带开放泛型形参；这类类型没有单一物理布局，只有语义身份。
fn contains_open_generic(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Generic(_) => true,
        ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(contains_open_generic),
        ValkyrieType::Apply(base, arguments) => contains_open_generic(base) || arguments.iter().any(contains_open_generic),
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => contains_open_generic(element),
        ValkyrieType::FixedArray { element, .. } => contains_open_generic(element),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
            items.iter().any(contains_open_generic)
        }
        ValkyrieType::Function(function) => {
            function.params.iter().any(contains_open_generic) || contains_open_generic(&function.return_type)
        }
        _ => false,
    }
}
