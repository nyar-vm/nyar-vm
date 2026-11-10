use std::collections::BTreeMap;

use nyar_types::{
    CanonicalPrimitiveType, CanonicalTypeKind, FieldId, FieldRecord, Identifier, LinkedSemanticProgram, NominalInstanceId,
    NominalInstanceRecord, NominalValueSemantics, StructuredDiagnosticSet, TypeId, VariantId,
};

use crate::compiler::{
    emitter::frontend_contract::concretize_type,
    mir::{MirModule, MirSumDeclaration, MirSumVariant, instance_layout_for_type, instantiate_fields, layout_id_for_type},
    types::hir::{PrimitiveNumericShape, ValkyrieType, numeric_primitive_identity_shape},
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
        let ty = ValkyrieType::Nominal { declaration: item, arguments: Vec::new(), primitive_identity: aggregate.primitive_identity.clone() };
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
        // 声明级只登记无开放形参的聚合字段。泛型声明的字段类型含 binder，
        // 必须等 register_aggregate_nominal_instances 按具体实例代入后再登记 FieldRecord。
        // 否则 Generic{binder} 会被 intern 进 canonical 类型表，backend 无法处理。
        if aggregate.generics.is_empty() {
            for field in &aggregate.fields {
                if field_records.insert(field.id, FieldRecord { owner: nominal, ty: type_id(types, &field.ty)? }).is_some() {
                    return Err(error_without_module("CAN022", format!("字段 identity 重复: {qualified}.{}", field.name)));
                }
            }
        }
    }
    for sum in &module.sum_types {
        let item = sum.declaration.ok_or_else(|| error_without_module("CAN018", format!("sum `{}` 缺少 declaration ItemId", sum.name)))?;
        let ty = ValkyrieType::Nominal { declaration: item, arguments: Vec::new(), primitive_identity: None };
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
/// 再冻结为独立 `FieldId` 与 `FieldRecord`。已闭合的实例额外登记实例级物理布局；
/// 仍含开放形参的实例只有语义身份与字段身份，禁止复用声明布局来伪造偏移。
/// 代入后的字段类型没有类型事实时失败关闭。
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
        .chain(
            module.sum_types.iter().flat_map(|sum| sum.variants.iter().flat_map(|variant| variant.fields.iter().map(|field| field.id.index()))),
        )
        .max()
        .map(|index| index + 1)
        .unwrap_or(0);
    for structure in &module.structs {
        let Some(declaration) = structure.declaration
        else {
            continue;
        };
        let declared = ValkyrieType::Nominal { declaration, arguments: Vec::new(), primitive_identity: structure.primitive_identity.clone() };
        let declaration_type = type_id(type_values, &declared)?;
        let Some(nominal) = declaration_nominals.get(&declaration_type).copied()
        else {
            return Err(error_without_module("CAN058", format!("聚合 `{}` 缺少声明级 nominal 身份", structure.qualified_name())));
        };
        // 声明侧字段身份恒等保留：单态聚合与未实例化路径继续使用声明字段。
        for field in &structure.fields {
            bindings.field_by_owner.insert((nominal, field.id), field.id);
        }
        bindings.by_type.insert(declaration_type, nominal);
        if structure.generics.is_empty() {
            continue;
        }
        let semantics = {
            let Some(semantics) = nominals.get(&nominal).map(|(_, semantics)| *semantics)
            else {
                return Err(error_without_module("CAN056", format!("聚合 `{}` 缺少冻结的 NominalValueSemantics", structure.qualified_name())));
            };
            semantics
        };
        for (concrete, concrete_type) in type_values {
            let ValkyrieType::Nominal { declaration: instance_declaration, arguments, .. } = concrete
            else {
                continue;
            };
            if *instance_declaration != declaration {
                continue;
            }
            // 声明本身（零实参）不重复铸造身份。
            if arguments.is_empty() {
                continue;
            }
            // 物理布局只对完全闭合的实例成立。仍含开放形参的实例（泛型声明体内对
            // 自身的引用、或由别的声明 binder 参数化的引用）只有语义身份与实例级
            // 字段身份；目标层真要布局时仍失败关闭，不在此猜偏移。
            let layout = if contains_open_generic(concrete) {
                None
            }
            else {
                Some(instance_layout_for_type(&mut linked.aggregate_layouts, structure, concrete, item_spellings).ok_or_else(|| {
                    error_without_module("CAN058", format!("聚合实例 `{}` 无法按已冻结声明生成实例级布局", structure.qualified_name()))
                })?)
            };
            let field_types = match &layout {
                Some(layout) => layout.field_types.clone(),
                None => instantiate_fields(structure, concrete).ok_or_else(|| {
                    error_without_module("CAN058", format!("聚合实例 `{}` 无法按已冻结声明代入字段类型", structure.qualified_name()))
                })?,
            };
            let layout_id = layout.as_ref().map(|layout| layout.id);
            let instance =
                NominalInstanceId::from_index(next_nominal).ok_or_else(|| error_without_module("CAN020", "聚合 instance identity 溢出"))?;
            next_nominal = next_nominal.checked_add(1).ok_or_else(|| error_without_module("CAN020", "聚合 instance identity 溢出"))?;
            nominals.insert(instance, (declaration_type, semantics));
            let mut instance_fields = Vec::with_capacity(structure.fields.len());
            for (index, (declared, field)) in structure.fields.iter().zip(field_types.iter()).enumerate() {
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
                if let Some(layout_id) = layout_id {
                    linked.aggregate_layout_by_field.insert(id, (layout_id, index as u32));
                }
            }
            linked.nominal_instances.insert(
                instance,
                NominalInstanceRecord { declaration: declaration_type, ty: *concrete_type, semantics, fields: instance_fields },
            );
            if let Some(layout_id) = layout_id {
                linked.aggregate_layout_by_type.insert(*concrete_type, layout_id);
                linked.aggregate_layout_by_nominal.insert(instance, layout_id);
            }
            bindings.by_type.insert(*concrete_type, instance);
        }
    }
    Ok(bindings)
}

/// 实例字段类型 intern 的总量上限。
///
/// `A<A<A<…>>>` 型嵌套实例每轮固定点必新增一层派生类型，总量无界即病态输入；
/// 正常闭包的 canonical 类型表远低于该量级。上限处 fail-closed，不许静默截断。
pub(crate) const MAX_INTERNED_TYPES: usize = 65_536;

/// 为实例级字段类型补齐 canonical 类型身份。
///
/// 实例字段类型是「声明字段 × binder 代入」的派生事实，可能从未作为 SSA 值出现，
/// 因此不在前端 `type_identities` 里。这里按既有子类型递归登记，新 `TypeId` 稠密
/// 追加在末尾，不改写任何前端已给出的身份；嵌套实例会引出新的实例类型，故循环到
/// 不再产出新类型为止。无法无损投影到 canonical 的派生类型仍失败关闭。
///
/// 循环只有「无增长」一个正常退出；总量超过 [`MAX_INTERNED_TYPES`] 判定为不收敛
/// 的嵌套实例（`A<A<A<…>>>`），按 CAN059 结构化失败并给出模块与当前规模上下文。
pub(crate) fn intern_instance_field_types(
    module: &MirModule,
    type_values: &mut BTreeMap<ValkyrieType, TypeId>,
) -> Result<(), StructuredDiagnosticSet> {
    let mut next = type_values.values().map(|id| id.index()).max().map(|index| index + 1).unwrap_or(0);
    loop {
        let before = type_values.len();
        let snapshot: Vec<ValkyrieType> = type_values.keys().cloned().collect();
        for structure in &module.structs {
            let Some(declaration) = structure.declaration
            else {
                continue;
            };
            if structure.generics.is_empty() {
                continue;
            }
            for concrete in &snapshot {
                let ValkyrieType::Nominal { declaration: instance_declaration, arguments, .. } = concrete
                else {
                    continue;
                };
                if *instance_declaration != declaration || arguments.is_empty() {
                    continue;
                }
                let Some(fields) = instantiate_fields(structure, concrete)
                else {
                    return Err(error_without_module(
                        "CAN058",
                        format!("聚合实例 `{}` 无法按已冻结声明代入字段类型", structure.qualified_name()),
                    ));
                };
                for field in &fields {
                    intern_type(type_values, field, &mut next)?;
                }
            }
            if type_values.len() > MAX_INTERNED_TYPES {
                return Err(error(
                    module,
                    "CAN059",
                    format!(
                        "实例字段类型 intern 不收敛：类型表规模 {} 超过上限 {MAX_INTERNED_TYPES}；聚合 `{}` 的嵌套实例持续派生新类型（`A<A<A<…>>>` 型输入须由上游拒绝）",
                        type_values.len(),
                        structure.qualified_name(),
                    ),
                ));
            }
        }
        if type_values.len() == before {
            break;
        }
    }
    Ok(())
}

/// 为多字段变体记录补齐 canonical 类型身份。
///
/// 记录身份 = 变体自身冻结 `ItemId` + 枚举实参。声明记录（零实参）与每个具体实例记录
/// 都必须进类型表；记录字段是已按 binder 代入的变体字段类型，也要 intern，供
/// `FieldRecord` 与随后生成的 `TypeDef` / 布局引用。单字段 payload 不是记录，跳过。
pub(crate) fn intern_variant_record_types(
    module: &MirModule,
    type_values: &mut BTreeMap<ValkyrieType, TypeId>,
) -> Result<(), StructuredDiagnosticSet> {
    let mut next = type_values.values().map(|id| id.index()).max().map(|index| index + 1).unwrap_or(0);
    let snapshot: Vec<ValkyrieType> = type_values.keys().cloned().collect();
    let mut records: Vec<(ValkyrieType, Vec<ValkyrieType>)> = Vec::new();
    for sum in &module.sum_types {
        let Some(declaration) = sum.declaration
        else {
            continue;
        };
        for concrete in &snapshot {
            let ValkyrieType::Nominal { declaration: instance_declaration, arguments, .. } = concrete
            else {
                continue;
            };
            if *instance_declaration != declaration {
                continue;
            }
            for variant in &sum.variants {
                if variant.fields.len() < 2 {
                    continue;
                }
                let Some(record_declaration) = variant.declaration
                else {
                    continue;
                };
                let record = ValkyrieType::Nominal { declaration: record_declaration, arguments: arguments.clone(), primitive_identity: None };
                let Some(fields) = sum.instantiate_field_types(variant, arguments)
                else {
                    continue;
                };
                records.push((record, fields));
            }
        }
    }
    for (record, fields) in records {
        intern_type(type_values, &record, &mut next)?;
        for field in fields {
            intern_type(type_values, &field, &mut next)?;
        }
    }
    Ok(())
}

///
/// 为每个聚合声明显式登记零实参 Nominal 类型到 type_values。
///
/// 泛型声明（`generics` 非空）的零实参 Nominal 是声明级身份：backend 需要它找到
/// declaration ItemId 来关联完整声明。`type_identity_table` 在正常路径上已经登记
/// 这条键，但跨模块引用或外部 std 依赖可能在链接后的合并表里丢失；这里在
/// `collect_aggregate_identities` 之前做最后一道防线——缺了就按稠密 `TypeId` 追加。
/// 非泛型声明同理统一处理，不做条件分支。
pub(crate) fn ensure_declaration_aggregate_types(
    module: &MirModule,
    type_values: &mut BTreeMap<ValkyrieType, TypeId>,
) -> Result<(), StructuredDiagnosticSet> {
    let mut next = type_values.values().map(|id| id.index()).max().map(|index| index + 1).unwrap_or(0);
    for aggregate in &module.structs {
        let Some(declaration) = aggregate.declaration
        else {
            continue;
        };
        let ty = ValkyrieType::Nominal { declaration, arguments: Vec::new(), primitive_identity: aggregate.primitive_identity.clone() };
        if type_values.contains_key(&ty) {
            continue;
        }
        intern_type(type_values, &ty, &mut next)?;
    }
    for sum in &module.sum_types {
        let Some(declaration) = sum.declaration
        else {
            continue;
        };
        let ty = ValkyrieType::Nominal { declaration, arguments: Vec::new(), primitive_identity: None };
        if type_values.contains_key(&ty) {
            continue;
        }
        intern_type(type_values, &ty, &mut next)?;
    }
    Ok(())
}

/// 递归登记缺失的派生类型；子类型先登记，保证新 id 的 canonical kind 引用完整。
fn intern_type(ids: &mut BTreeMap<ValkyrieType, TypeId>, ty: &ValkyrieType, next: &mut u32) -> Result<(), StructuredDiagnosticSet> {
    if type_id(ids, ty).is_ok() {
        return Ok(());
    }
    match ty {
        ValkyrieType::Nominal { arguments, .. }
        | ValkyrieType::Tuple(arguments)
        | ValkyrieType::Union(arguments)
        | ValkyrieType::Intersection(arguments) => {
            for argument in arguments {
                intern_type(ids, argument, next)?;
            }
        }
        ValkyrieType::Apply(base, arguments) => {
            intern_type(ids, base, next)?;
            for argument in arguments {
                intern_type(ids, argument, next)?;
            }
        }
        ValkyrieType::LabeledTuple(fields) => {
            for field in fields {
                intern_type(ids, &field.ty, next)?;
            }
        }
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => intern_type(ids, element, next)?,
        ValkyrieType::FixedArray { element, .. } => intern_type(ids, element, next)?,
        ValkyrieType::Function(function) => {
            for parameter in &function.params {
                intern_type(ids, parameter, next)?;
            }
            intern_type(ids, &function.return_type, next)?;
        }
        _ => {}
    }
    canonical_type_kind(ty, ids)?;
    let id = TypeId::from_index(*next).ok_or_else(|| error_without_module("CAN014", format!("实例字段类型 identity 溢出: {ty:?}")))?;
    *next = (*next).checked_add(1).ok_or_else(|| error_without_module("CAN014", format!("实例字段类型 identity 溢出: {ty:?}")))?;
    ids.insert(ty.clone(), id);
    Ok(())
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
        // 身份驱动：数组下标只认声明携带的 `[primitive]` 整数身份。
        // 类型自身缺身份时回查声明侧 `MirStruct`（同一注解事实），禁止按名字认。
        ValkyrieType::Nominal { declaration, arguments, primitive_identity } if arguments.is_empty() => {
            let identity = primitive_identity.as_deref().or_else(|| {
                module.structs.iter().find(|item| item.declaration == Some(*declaration)).and_then(|item| item.primitive_identity.as_deref())
            });
            identity.and_then(numeric_primitive_identity_shape).is_some_and(|shape| matches!(shape, PrimitiveNumericShape::Integer { .. }))
        }
        _ => false,
    }
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
        ValkyrieType::Nominal { declaration: item, arguments, primitive_identity } => {
            let declaration_ty =
                ValkyrieType::Nominal { declaration: *item, arguments: Vec::new(), primitive_identity: primitive_identity.clone() };
            let declaration = type_id(ids, &declaration_ty)?;
            CanonicalTypeKind::Nominal { declaration, arguments: arguments.iter().map(id).collect::<Result<_, _>>()? }
        }
        ValkyrieType::Named(name) => {
            return Err(error_without_module(
                "CAN003",
                format!("未绑定的类型名 `{name}` 进入 canonical 类型表；必须在 HIR 绑定阶段冻结为 Nominal 或返回前端 TypeError"),
            ));
        }
        // `Apply` 必须在 HIR 绑定阶段折成唯一结构形态（`Nominal` / `Array` / `FixedArray`）。
        // 到达 canonical 说明上游漏折，按缺陷拒绝，不在本层兜成 `Nominal`。
        ValkyrieType::Apply(base, _) => {
            return Err(error_without_module(
                "CAN003",
                format!("未折叠的类型应用 `{base:?}` 进入 canonical 类型表；必须在 HIR 绑定阶段折成 Nominal 或 Array"),
            ));
        }
        ValkyrieType::Tuple(items) => CanonicalTypeKind::Tuple(items.iter().map(id).collect::<Result<_, _>>()?),
        // 标签是类型身份的一部分：`LabeledTuple` 登记为独立 kind，不折成 `Tuple`。
        // 字段顺序即位置身份，与 `CanonicalTypeKind::LabeledTuple` 的定义逐字段一致。
        ValkyrieType::LabeledTuple(fields) => {
            let mut labeled = Vec::with_capacity(fields.len());
            for field in fields {
                labeled.push((field.name.clone(), id(&field.ty)?));
            }
            CanonicalTypeKind::LabeledTuple { fields: labeled }
        }
        ValkyrieType::Array(element) => CanonicalTypeKind::Array { element: id(element)?, length: None },
        ValkyrieType::FixedArray { element, length } => CanonicalTypeKind::Array { element: id(element)?, length: Some(*length as u64) },
        ValkyrieType::Nullable(element) => CanonicalTypeKind::Nullable(id(element)?),
        ValkyrieType::Union(items) => CanonicalTypeKind::Union(items.iter().map(id).collect::<Result<_, _>>()?),
        ValkyrieType::Intersection(items) => CanonicalTypeKind::Intersection(items.iter().map(id).collect::<Result<_, _>>()?),
        ValkyrieType::Function(function) => CanonicalTypeKind::Function {
            parameters: function.params.iter().map(id).collect::<Result<_, _>>()?,
            return_type: id(&function.return_type)?,
        },
        // 泛型形参：带 `binder` 的声明身份是稳定的类型事实（泛型函数签名里的形参）。
        // 无 binder 的形参仍是未解析占位，不得进入 canonical。
        ValkyrieType::Generic(generic) => {
            let Some(binder) = generic.binder
            else {
                return Err(error_without_module("CAN003", format!("泛型形参 `{}` 缺少 binder 身份，不得进入 canonical 类型表", generic.name)));
            };
            CanonicalTypeKind::Generic { owner: binder.owner, ordinal: binder.ordinal }
        }
        _ => {
            return Err(error_without_module("CAN003", format!("类型没有无损 canonical 形状: {ty:?}")));
        }
    })
}

/// 多字段变体记录在 canonical 侧的冻结身份。
///
/// 同一枚举的两个变体各有独立记录身份（`variant.declaration` 不同），不共享一个。
#[derive(Debug, Clone)]
pub(crate) struct VariantRecordBinding {
    /// 记录自身的 nominal 实例身份。
    pub nominal: NominalInstanceId,
    /// 记录类型在 canonical 类型表中的 `TypeId`。
    pub ty: TypeId,
    /// 记录字段的声明身份，按变体声明顺序排列（canonical 再映射到实例字段身份）。
    pub fields: Vec<FieldId>,
}

/// 登记一个多字段变体记录：记录 nominal、记录字段身份与布局绑定。
///
/// 记录身份 = 变体自身冻结 `ItemId` + 枚举实参。记录只有一条布局链路：
/// `ValkyrieType::Nominal` → 已登记 `AggregateLayout` → `FieldGet` 偏移。
#[allow(clippy::too_many_arguments)]
fn register_variant_record(
    sum: &MirSumDeclaration,
    variant: &MirSumVariant,
    arguments: &[ValkyrieType],
    type_values: &BTreeMap<ValkyrieType, TypeId>,
    linked: &mut LinkedSemanticProgram,
    nominals: &mut BTreeMap<NominalInstanceId, AggregateIdentity>,
    field_records: &mut BTreeMap<FieldId, FieldRecord>,
    field_by_owner: &mut BTreeMap<(NominalInstanceId, FieldId), FieldId>,
    next_nominal: &mut u32,
    next_field: &mut u32,
) -> Result<Option<VariantRecordBinding>, StructuredDiagnosticSet> {
    if variant.fields.len() < 2 {
        return Ok(None);
    }
    let declaration =
        variant.declaration.ok_or_else(|| error_without_module("CAN018", format!("变体 `{}` 缺少 declaration ItemId", variant.name)))?;
    let record_ty = ValkyrieType::Nominal { declaration, arguments: arguments.to_vec(), primitive_identity: None };
    let record_type = type_id(type_values, &record_ty)?;
    let field_types = sum
        .instantiate_field_types(variant, arguments)
        .ok_or_else(|| error_without_module("CAN018", format!("变体 `{}` 的记录字段无法按枚举实参代入", variant.name)))?;
    let record =
        NominalInstanceId::from_index(*next_nominal).ok_or_else(|| error_without_module("CAN020", "变体记录 instance identity 溢出"))?;
    *next_nominal = (*next_nominal).checked_add(1).ok_or_else(|| error_without_module("CAN020", "变体记录 instance identity 溢出"))?;
    nominals.insert(record, (record_type, NominalValueSemantics::Value));
    let layout_id = layout_id_for_type(&record_ty, &linked.aggregate_layouts);
    let mut ids = Vec::with_capacity(variant.fields.len());
    for (index, (field, ty)) in variant.fields.iter().zip(field_types.iter()).enumerate() {
        let id = FieldId::from_index(*next_field).ok_or_else(|| error_without_module("CAN022", "变体记录字段 identity 溢出"))?;
        *next_field = (*next_field).checked_add(1).ok_or_else(|| error_without_module("CAN022", "变体记录字段 identity 溢出"))?;
        if field_records.insert(id, FieldRecord { owner: record, ty: type_id(type_values, ty)? }).is_some() {
            return Err(error_without_module("CAN022", format!("变体记录字段 identity 冲突: {}::{}.{}", sum.name, variant.name, field.name)));
        }
        field_by_owner.insert((record, field.id), id);
        if let Some(layout_id) = layout_id {
            linked.aggregate_layout_by_field.insert(id, (layout_id, index as u32));
        }
        ids.push(id);
    }
    linked.nominal_instances.insert(
        record,
        NominalInstanceRecord { declaration: record_type, ty: record_type, semantics: NominalValueSemantics::Value, fields: ids },
    );
    if let Some(layout_id) = layout_id {
        linked.aggregate_layout_by_type.insert(record_type, layout_id);
        linked.aggregate_layout_by_nominal.insert(record, layout_id);
    }
    Ok(Some(VariantRecordBinding { nominal: record, ty: record_type, fields: variant.fields.iter().map(|field| field.id).collect() }))
}

/// 为每个具体 `Nominal{ItemId, args}` 注册 sum 实例与已代入 payload；SumNew 按结果类型选取实例。
pub(crate) fn register_sum_nominal_instances(
    module: &MirModule,
    type_values: &BTreeMap<ValkyrieType, TypeId>,
    declaration_spellings: &BTreeMap<TypeId, Identifier>,
    linked: &mut LinkedSemanticProgram,
    nominals: &mut BTreeMap<NominalInstanceId, AggregateIdentity>,
    field_records: &mut BTreeMap<FieldId, FieldRecord>,
    field_by_owner: &mut BTreeMap<(NominalInstanceId, FieldId), FieldId>,
    variant_records: &mut BTreeMap<(NominalInstanceId, VariantId), VariantRecordBinding>,
) -> Result<BTreeMap<TypeId, NominalInstanceId>, StructuredDiagnosticSet> {
    let mut by_type = BTreeMap::new();
    let mut next_nominal = nominals.keys().map(|id| id.index()).max().map(|index| index + 1).unwrap_or(0);
    // 变体记录字段身份与聚合字段同表稠密互异，必须在全部声明字段之后分配。
    let mut next_field = field_records
        .keys()
        .map(|field| field.index())
        .chain(module.structs.iter().flat_map(|structure| structure.fields.iter().map(|field| field.id.index())))
        .chain(
            module.sum_types.iter().flat_map(|sum| sum.variants.iter().flat_map(|variant| variant.fields.iter().map(|field| field.id.index()))),
        )
        .max()
        .map(|index| index + 1)
        .unwrap_or(0);
    for sum in &module.sum_types {
        let item = sum.declaration.ok_or_else(|| error_without_module("CAN018", format!("sum `{}` 缺少 declaration ItemId", sum.name)))?;
        let declaration_ty = ValkyrieType::Nominal { declaration: item, arguments: Vec::new(), primitive_identity: None };
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
        // 声明级 sum 布局同样走 `sum_variant_layout_payload`：单字段名义 payload（`utf8` 即
        // `Utf8Text` 声明）按已冻结声明拼写结算，与具体实例共用同一条派生链路。
        // 开放泛型声明不携带未代入 payload（避免 binder 进入布局）。
        let mut layout_variants = Vec::with_capacity(sum.variants.len());
        for variant in &sum.variants {
            let instantiated = if sum.generics.is_empty() {
                sum.instantiate_payload(variant, &[])
                    .ok_or_else(|| error_without_module("CAN018", format!("sum `{}` 实例化 payload 失败", sum.name)))?
            }
            else {
                None
            };
            let payload_type = instantiated.as_ref().map(|ty| type_id(type_values, ty)).transpose()?;
            linked.variants.insert((sum.nominal, variant.id), nyar_types::VariantRecord { payload_type });
            // 多字段 payload 是变体记录：声明记录进类型表、字段进 field_by_owner，并由记录布局生成 TypeDef。
            if sum.generics.is_empty() {
                if let Some(binding) = register_variant_record(
                    sum,
                    variant,
                    &[],
                    type_values,
                    linked,
                    nominals,
                    field_records,
                    field_by_owner,
                    &mut next_nominal,
                    &mut next_field,
                )? {
                    // 声明侧记录：SumNew 该枚举的结果 nominal 就是 `sum.nominal`。
                    by_type.insert(binding.ty, binding.nominal);
                    variant_records.insert((sum.nominal, variant.id), binding);
                }
            }
            let layout_payload = match instantiated.as_ref() {
                Some(ty) => {
                    Some(sum_variant_layout_payload(variant, ty, type_values, declaration_spellings, &linked.aggregate_layouts).ok_or_else(
                        || error_without_module("CAN054", format!("sum `{}` 变体 `{}` 的 payload 类型未完成实例化", sum.name, variant.name)),
                    )?)
                }
                None => None,
            };
            layout_variants.push(nyar_types::SumVariantLayout {
                id: variant.id,
                name: variant.name.clone(),
                tag: variant.tag,
                payload_type: layout_payload,
            });
        }
        linked.sum_types.push(nyar_types::SumTypeLayout {
            nominal: sum.nominal,
            name: sum.name.clone(),
            is_unite: sum.is_unite,
            tag_width: 4,
            variants: layout_variants,
        });
        by_type.insert(declaration, sum.nominal);
    }
    for (ty, concrete_ty) in type_values {
        let ValkyrieType::Nominal { declaration: item, arguments, .. } = ty
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
        let declaration = type_id(type_values, &ValkyrieType::Nominal { declaration: *item, arguments: Vec::new(), primitive_identity: None })?;
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
            // 泛型实例的变体记录同样按「变体 ItemId + 枚举实参」独立登记：同一枚举的两个
            // 变体各有记录身份，不共享一个。
            if let Some(binding) = register_variant_record(
                sum,
                variant,
                arguments,
                type_values,
                linked,
                nominals,
                field_records,
                field_by_owner,
                &mut next_nominal,
                &mut next_field,
            )? {
                // 具体泛型实例记录：SumNew 的结果 nominal 成败在此一举，记录同样按实例独立。
                by_type.insert(binding.ty, binding.nominal);
                variant_records.insert((nominal, variant.id), binding);
            }
            // 布局侧仍写 NyarType；语义身份只认上方 VariantRecord 的 TypeId。
            // 多字段 payload 走变体记录已登记布局；单字段名义 payload（`usize`、
            // `SwissTableEntry<K, V>` 等）没有结构化目标形状，按声明身份已冻结的
            // surface spelling 表示，与 emitter `lower_type` 同口径。
            let layout_payload = instantiated
                .as_ref()
                .and_then(|ty| sum_variant_layout_payload(variant, ty, type_values, declaration_spellings, &linked.aggregate_layouts));
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

/// sum variant payload 的布局侧形状：声明级与具体实例共用的唯一派生点。
///
/// 多字段 payload 是变体记录，只认已登记布局；单字段 payload 优先按已登记声明布局
/// 携带 `LayoutId`，未登记时按已冻结声明拼写保留残差。两者都不在调用点按分支补特判。
/// 返回 `None` 由调用方失败关闭。
fn sum_variant_layout_payload(
    variant: &MirSumVariant,
    instantiated: &ValkyrieType,
    type_values: &BTreeMap<ValkyrieType, TypeId>,
    declaration_spellings: &BTreeMap<TypeId, Identifier>,
    layouts: &nyar_types::AggregateLayoutPlan,
) -> Option<nyar_types::NyarType> {
    if variant.fields.len() > 1 {
        crate::compiler::mir::record_payload_layout(instantiated, layouts)
    }
    else {
        sum_payload_layout(instantiated, type_values, declaration_spellings, layouts)
    }
}

/// 单字段 payload 的布局拼写结算（`sum_variant_layout_payload` 的单字段分支）。
///
/// 语义身份只认 `VariantRecord::payload_type`；这里只回答 RepresentationPlan 与物理
/// 布局需要的 `NyarType`。结构化 payload 走严格 `concretize_type`；名义 payload
/// 优先查已登记的声明布局并携带 `LayoutId`，未登记时才保留 surface spelling 残差，
/// 由预发射门禁拒绝，不按名字猜实现。
fn sum_payload_layout(
    ty: &ValkyrieType,
    type_values: &BTreeMap<ValkyrieType, TypeId>,
    declaration_spellings: &BTreeMap<TypeId, Identifier>,
    layouts: &nyar_types::AggregateLayoutPlan,
) -> Option<nyar_types::NyarType> {
    if let Ok(concrete) = concretize_type(ty) {
        return Some(concrete);
    }
    let ValkyrieType::Nominal { declaration, arguments, primitive_identity } = ty
    else {
        return None;
    };
    let declaration_id = type_values.get(&ValkyrieType::Nominal {
        declaration: *declaration,
        arguments: Vec::new(),
        primitive_identity: primitive_identity.clone(),
    })?;
    let spelling = declaration_spellings.get(declaration_id)?;
    if arguments.is_empty() {
        // 名义声明若已有冻结布局，必须带 `LayoutId`；裸短名会被预发射门禁拒绝。
        if let Some(primitive) = nyar_types::NyarType::from_surface_primitive(spelling.as_str()) {
            return Some(primitive);
        }
        if let Some(layout) = crate::compiler::mir::layout_id_for_type(ty, layouts) {
            return Some(nyar_types::NyarType::Nominal { layout, spelling: spelling.clone() });
        }
        return Some(nyar_types::NyarType::Named(spelling.clone()));
    }
    let layout =
        nyar_types::NyarType::from_surface_primitive(spelling.as_str()).unwrap_or_else(|| nyar_types::NyarType::Named(spelling.clone()));
    Some(nyar_types::NyarType::Apply(
        Box::new(layout),
        arguments
            .iter()
            .map(|argument| sum_payload_layout(argument, type_values, declaration_spellings, layouts))
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
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => items.iter().any(contains_open_generic),
        ValkyrieType::Function(function) => function.params.iter().any(contains_open_generic) || contains_open_generic(&function.return_type),
        _ => false,
    }
}
