use std::collections::BTreeMap;

use nyar_types::{
    CanonicalPrimitiveType, CanonicalTypeKind, FieldId, FieldRecord, LinkedSemanticProgram, NominalInstanceId, NominalInstanceRecord,
    NominalValueSemantics, StructuredDiagnosticSet, TypeId,
};

use crate::compiler::{mir::MirModule, types::hir::ValkyrieType};

use super::errors::{error, error_without_module, type_id};

pub(crate) type AggregateIdentity = (TypeId, NominalValueSemantics);

pub(crate) fn collect_aggregate_identities(
    module: &MirModule,
    types: &BTreeMap<ValkyrieType, TypeId>,
) -> Result<(BTreeMap<NominalInstanceId, AggregateIdentity>, BTreeMap<FieldId, FieldRecord>), StructuredDiagnosticSet> {
    let mut nominals = BTreeMap::new();
    let mut field_records = BTreeMap::new();
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
    }
    Ok((nominals, field_records))
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
        ValkyrieType::Named(name) => {
            return Err(error_without_module(
                "CAN003",
                format!("未绑定的类型名 `{name}` 进入 canonical 类型表；必须在 HIR 绑定阶段冻结为 Nominal 或返回前端 TypeError"),
            ));
        }
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
        _ => {
            return Err(error_without_module("CAN003", format!("类型没有无损 canonical 形状: {ty:?}")));
        }
    })
}

/// 为每个具体 `Nominal{ItemId, args}` 注册 sum 实例与已代入 payload；SumNew 按结果类型选取实例。
pub(crate) fn register_sum_nominal_instances(
    module: &MirModule,
    type_values: &BTreeMap<ValkyrieType, TypeId>,
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
        let mut layout_variants = Vec::with_capacity(sum.variants.len());
        for variant in &sum.variants {
            let instantiated = sum
                .instantiate_payload(variant, arguments)
                .ok_or_else(|| error_without_module("CAN018", format!("sum `{}` 实例化 payload 失败", sum.name)))?;
            let payload_type = instantiated.as_ref().map(|ty| type_id(type_values, ty)).transpose()?;
            linked.variants.insert((nominal, variant.id), nyar_types::VariantRecord { payload_type });
            // 布局侧仍写 NyarType；语义身份只认上方 VariantRecord 的 TypeId。
            let layout_payload = instantiated
                .as_ref()
                .map(crate::compiler::emitter::frontend_contract::concretize_type)
                .transpose()
                .map_err(|failure| error_without_module("CAN054", format!("sum `{}` 实例 payload 无法 concretize: {failure}", sum.name)))?;
            layout_variants.push(nyar_types::SumVariantLayout {
                id: variant.id,
                name: variant.name.clone(),
                tag: variant.tag,
                payload_type: layout_payload,
            });
        }
        linked.sum_types.push(nyar_types::SumTypeLayout {
            nominal,
            name: sum.name.clone(),
            is_unite: sum.is_unite,
            tag_width: 4,
            variants: layout_variants,
        });
        by_type.insert(*concrete_ty, nominal);
    }
    Ok(by_type)
}
