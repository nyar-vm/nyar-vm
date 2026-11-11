//! Semantic MIR 的实例身份链接边界。
//!
//! 链接器只接受前端已经冻结的 ItemInstanceId 闭包，不从名称或布局补造身份。

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::{
    mir::{MirFunction, MirModule, MirOperand, MirOperation, MirStruct, MirSumDeclaration},
    syntax::oak::ParseError,
    types::hir::HirModule,
};
use nyar_types::{
    FieldId, ItemInstanceId, NyarFunctionType, NyarType, NominalInstanceId, WitnessObject,
    layout::{AggregateLayout, AggregateLayoutPlan, LayoutId},
};

use super::host_bindings::remap_call_targets;

/// 合并已由 Compiler 统一注册的依赖实例闭包。
///
/// 正常编译路径使用 [`link_reachable_dependency_mir_with_hir`]。
#[doc(hidden)]
pub fn link_reachable_dependency_mir(
    consumer: &mut MirModule,
    dependency_mirs: &[MirModule],
) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    link_reachable_dependency_mir_with_hir(consumer, dependency_mirs, &[], &BTreeMap::new())
}

/// 合并已由 Compiler 统一注册的依赖实例闭包。
///
/// `host_binding_remaps` 是本次编译中 `apply_host_provider_bindings` 已实际执行的
/// 合同→provider 调用改写表；首轮链接传空表。绑定会把消费模块内全部函数（含首轮
/// 链接迁入的依赖体副本）的 Call 目标改写为 provider 身份，二次链接据此把依赖原件
/// 规范化到同一绑定后状态，再与消费模块副本逐字段核对；规范化后仍不一致即真实冲突。
#[doc(hidden)]
pub fn link_reachable_dependency_mir_with_hir(
    consumer: &mut MirModule,
    dependency_mirs: &[MirModule],
    _hirs: &[&HirModule],
    host_binding_remaps: &BTreeMap<ItemInstanceId, ItemInstanceId>,
) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    if dependency_mirs.is_empty() {
        return Ok(());
    }
    let (structs, sum_types, remaps) = freeze_aggregate_identities(consumer, dependency_mirs)?;
    let aggregate_layouts = merge_aggregate_layouts(consumer, dependency_mirs)?;
    let mut definitions = BTreeMap::new();
    let mut imports = BTreeMap::new();
    for module in std::iter::once(&*consumer).chain(dependency_mirs) {
        if module.type_identities != consumer.type_identities {
            return Err(ParseError::Invalid { message: "依赖链接必须消费 Compiler 统一 TypeId 表".into(), span: None });
        }
        for function in &module.functions {
            if let Some(instance) = function.instance {
                if function.declaration.is_none() {
                    return Err(ParseError::Invalid {
                        message: format!(
                            "ItemInstanceId `{instance}` 缺少声明 ItemId；symbol=`{}`; module=`{}`",
                            function.display_symbol, module.name
                        ),
                        span: None,
                    });
                }
                if let Some((previous_module, previous)) = definitions.insert(instance, (module, function)) {
                    // 宿主绑定已把依赖 provider 定义迁入消费模块时，同一身份会在本模块与依赖
                    // 模块各有一份副本。唯一副本仍归消费模块；只接受逐字段完全一致的合同，
                    // 定义一旦不同就是真实冲突，仍然失败闭合。
                    let installed_copy = std::ptr::eq(previous_module, &*consumer) && !std::ptr::eq(module, &*consumer);
                    if installed_copy {
                        // 消费模块副本可能已被绑定改写 Call 目标（首轮链接迁入的依赖体
                        // 同样被重写），而依赖原件保持绑定前状态。用同一改写事实规范化
                        // 依赖原件后逐字段核对；规范化后仍不一致就是真实冲突。
                        let matches_copy = if previous == function {
                            true
                        }
                        else {
                            let mut normalized = (*function).clone();
                            // 依赖原件仍持有本地聚合身份；同一冻结身份核对前必须套用与
                            // linked 副本相同的聚合重编号与绑定后调用图事实。
                            let module_index = std::iter::once(&*consumer)
                                .chain(dependency_mirs)
                                .position(|candidate| std::ptr::eq(candidate, module))
                                .ok_or_else(|| ParseError::Invalid {
                                    message: "依赖模块身份不在当前源码闭包中".into(), span: None
                                })?;
                            remap_function_aggregates(&mut normalized, &remaps[module_index])?;
                            remap_call_targets(&mut normalized, host_binding_remaps);
                            &normalized == previous
                        };
                        if matches_copy {
                            definitions.insert(instance, (previous_module, previous));
                            continue;
                        }
                    }
                    return Err(ParseError::Invalid {
                        message: format!(
                            "ItemInstanceId `{instance}` 定义重复；first=`{}`@`{}`; second=`{}`@`{}`",
                            previous.display_symbol, previous_module.name, function.display_symbol, module.name
                        ),
                        span: None,
                    });
                }
            }
        }
        for contract in &module.external_calls {
            let instance =
                contract.instance.ok_or_else(|| ParseError::Invalid { message: "外部导入缺少 ItemInstanceId".into(), span: None })?;
            if contract.declaration.is_none() {
                return Err(ParseError::Invalid { message: "外部导入缺少 ItemId".into(), span: None });
            }
            if let Some(previous) = imports.insert(instance, contract)
                && previous != contract
            {
                return Err(ParseError::Invalid { message: format!("导入实例 `{instance}` 合同冲突"), span: None });
            }
        }
    }
    let mut linked = consumer.clone();
    linked.structs = structs;
    linked.sum_types = sum_types;
    linked.aggregate_layouts = aggregate_layouts;
    for function in &mut linked.functions {
        remap_function_aggregates(function, &remaps[0])?;
    }
    let mut visited = BTreeSet::new();
    // 链接根只取已冻结的出口/入口实例；泛型种子等不可达实例不参与传播，
    // 否则会拉入未闭合的 requirement 或无关依赖。无出口合同的分析用闭包
    // 以已实例化函数为根。
    let mut pending = consumer
        .exports
        .iter()
        .filter_map(|export| export.instance)
        .chain(consumer.entries.iter().filter_map(|entry| entry.instance))
        .collect::<Vec<_>>();
    if pending.is_empty() {
        pending = consumer.functions.iter().filter_map(|function| function.instance).collect();
    }
    // 消费模块自身发射的函数体里已冻结的调用目标同样是必须闭合的引用：
    // 出口可达链之外的声明体（如泛型种子声明）引用依赖侧铸造的开放克隆时，
    // 该实例必须随链接迁入消费模块，否则 `validate_static_call_resolution` 报 SMIR003。
    // 闭包值（`LoadFunctionRef`）虽然不经静态 `Call` 边调用，其代码指针同样是必须
    // 迁入的函数实例；漏掉该边会让 lambda 合成函数留在依赖模块，运行期间接调用无目标。
    for function in &consumer.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                match &instruction.kind {
                    MirOperation::Call { callee: MirOperand::Callable(callee), .. } => pending.push(*callee),
                    MirOperation::LoadFunctionRef { instance, .. } => pending.push(*instance),
                    _ => {}
                }
            }
        }
    }
    while let Some(instance) = pending.pop() {
        if !visited.insert(instance) {
            continue;
        }
        if let Some((module, function)) = definitions.get(&instance) {
            // 依赖原件仍持有绑定前的调用图：先按已生效改写表规范化，再用同一副本扫边。
            // 只做 push 侧的聚合重映射不足以阻断 BFS——它仍会沿合同边把 `host()` 空链接
            // 原样推进 external_calls，后端随后拒绝编码。consumer 自有函数已由
            // `apply_host_provider_bindings` 改写，二次套用会搬移已冻结的聚合身份。
            let from_dependency = !std::ptr::eq(*module, &*consumer);
            let mut normalized = (*function).clone();
            if from_dependency {
                let module_index = std::iter::once(&*consumer)
                    .chain(dependency_mirs)
                    .position(|candidate| std::ptr::eq(candidate, *module))
                    .ok_or_else(|| ParseError::Invalid { message: "依赖模块身份不在当前源码闭包中".into(), span: None })?;
                remap_function_aggregates(&mut normalized, &remaps[module_index])?;
                remap_call_targets(&mut normalized, host_binding_remaps);
            }
            for block in &normalized.blocks {
                for instruction in &block.instructions {
                    match &instruction.kind {
                        MirOperation::Call { callee: MirOperand::Callable(callee), .. } => pending.push(*callee),
                        MirOperation::LoadFunctionRef { instance, .. } => pending.push(*instance),
                        _ => {}
                    }
                }
            }
            if from_dependency {
                let symbol = format!("{}#{}", normalized.display_symbol, instance.index());
                if let Err(detail) = linked.register_callable_symbol(instance, symbol) {
                    return Err(ParseError::Invalid { message: detail, span: None });
                }
                linked.functions.push(normalized);
            }
        }
        else if let Some(contract) = imports.get(&instance) {
            if !linked.external_calls.contains(contract) {
                linked.external_calls.push((*contract).clone());
                let base = crate::compiler::hir::symbols::stable_symbol_key(&contract.symbol);
                let symbol = format!("{base}#{}", instance.index());
                if let Err(detail) = linked.register_callable_symbol(instance, symbol) {
                    return Err(ParseError::Invalid { message: detail, span: None });
                }
            }
        }
        else {
            let callers = consumer
                .functions
                .iter()
                .chain(dependency_mirs.iter().flat_map(|module| module.functions.iter()))
                .filter(|function| {
                    function.blocks.iter().any(|block| {
                        block.instructions.iter().any(|instruction| {
                            matches!(
                                &instruction.kind,
                                MirOperation::Call { callee: MirOperand::Callable(callee), .. } if *callee == instance
                            )
                        })
                    })
                })
                .map(|function| function.display_symbol.clone())
                .collect::<Vec<_>>();
            let local_imports = consumer.external_calls.iter().map(|c| c.symbol.to_string()).take(12).collect::<Vec<_>>();
            return Err(ParseError::Invalid {
                message: format!("调用实例 `{instance}` 没有定义或显式导入合同；callers={callers:?}; consumer_imports={local_imports:?}"),
                span: None,
            });
        }
    }
    *consumer = linked;
    Ok(())
}

/// 把入口组的公开表面并入唯一程序模块。
///
/// 程序模块是全部入口组共同的链接根宿主。合并只按已冻结的合同身份进行：
/// 同一实例的导出/入口必须完全一致；同一片段身份只允许 union；片段
/// 入口 operation 或 wasm 导出名冲突必须失败，不能择一覆盖。
#[doc(hidden)]
pub fn merge_entry_group_surface(consumer: &mut MirModule, entry: &MirModule) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    for contract in &entry.exports {
        match consumer.exports.iter().find(|existing| existing.instance == contract.instance) {
            Some(existing) if existing != contract => {
                return Err(ParseError::Invalid { message: format!("入口组导出合同冲突: `{}`", contract.symbol), span: None });
            }
            Some(_) => {}
            None => consumer.exports.push(contract.clone()),
        }
    }
    for contract in &entry.entries {
        match consumer.entries.iter().find(|existing| existing.instance == contract.instance) {
            Some(existing) if existing != contract => {
                return Err(ParseError::Invalid { message: format!("入口组入口合同冲突: `{}`", contract.symbol), span: None });
            }
            Some(_) => {}
            None => consumer.entries.push(contract.clone()),
        }
    }
    for fragment in &entry.semantic_fragments {
        let Some(existing) = consumer.semantic_fragments.iter_mut().find(|existing| existing.id == fragment.id)
        else {
            consumer.semantic_fragments.push(fragment.clone());
            continue;
        };
        for operation in &fragment.exported_operations {
            if !existing.exported_operations.contains(operation) {
                existing.exported_operations.push(*operation);
            }
        }
        for (instance, name) in &fragment.wasm_export_names {
            if let Some(previous) = existing.wasm_export_names.insert(*instance, name.clone())
                && previous != *name
            {
                return Err(ParseError::Invalid { message: format!("片段 `{}` 的 wasm 导出名冲突", fragment.id), span: None });
            }
        }
        match (existing.entry_operation, fragment.entry_operation) {
            (Some(previous), Some(next)) if previous != next => {
                return Err(ParseError::Invalid { message: format!("片段 `{}` 包含多个入口 operation", fragment.id), span: None });
            }
            (None, Some(next)) => existing.entry_operation = Some(next),
            _ => {}
        }
        for capability in &fragment.required_capabilities {
            if !existing.required_capabilities.contains(capability) {
                existing.required_capabilities.push(capability.clone());
            }
        }
        if existing.reference_management_hint.is_none() {
            existing.reference_management_hint = fragment.reference_management_hint.clone();
        }
    }
    Ok(())
}

/// 按 declaration identity / 合成结构键合并依赖的聚合布局；外部测试入口。
#[doc(hidden)]
pub fn merge_aggregate_layouts(consumer: &MirModule, dependencies: &[MirModule]) -> Result<AggregateLayoutPlan, ParseError> {
    let mut merged = consumer.aggregate_layouts.clone();
    for module in dependencies {
        let mut local_to_merged = BTreeMap::new();
        let mut next_id = merged.layouts.iter().map(|layout| layout.id).max().unwrap_or(0).saturating_add(1);
        for layout in &module.aggregate_layouts.layouts {
            let owner = declaration_for_layout(module, layout);
            let existing_id = match owner {
                Some(item) => merged.declaration_to_layout.get(&item).copied(),
                None => layout.key.as_ref().and_then(|key| merged.synthetic_layouts.get(key).copied()),
            };
            if owner.is_none() && layout.key.is_none() {
                return Err(ParseError::Invalid {
                    message: format!("布局 `{}` 缺少 declaration identity，禁止按名合并", layout_diagnostic_label(layout)),
                    span: None,
                });
            }
            let merged_id = match existing_id {
                Some(id) => id,
                None => {
                    let id = next_id;
                    next_id = next_id.saturating_add(1);
                    id
                }
            };
            local_to_merged.insert(layout.id, merged_id);
        }
        for layout in &module.aggregate_layouts.layouts {
            let merged_id = local_to_merged.get(&layout.id).copied().ok_or_else(|| ParseError::Invalid {
                message: format!("布局 `{}` 本地身份未登记", layout_diagnostic_label(layout)),
                span: None,
            })?;
            let normalized = normalize_layout_references(layout, &local_to_merged)?;
            if let Some(existing) = merged.layouts.iter().find(|candidate| candidate.id == merged_id) {
                if layout_contract_differs(existing, &normalized) {
                    let label = match layout.key.as_ref() {
                        Some(key) => format!("布局键 `{key:?}`"),
                        None => format!("布局 `{}`", layout_diagnostic_label(layout)),
                    };
                    return Err(ParseError::Invalid { message: format!("{label} 合同冲突"), span: None });
                }
                continue;
            }
            let owner = declaration_for_layout(module, layout);
            let mut copied = normalized;
            copied.id = merged_id;
            if let Some(item) = owner {
                // 只按声明身份合并；名称或物理形状相同不能补出语义身份。
                if merged.declaration_to_layout.insert(item, merged_id).is_some() {
                    return Err(ParseError::Invalid {
                        message: format!("布局 `{}` declaration identity 重复", layout_diagnostic_label(layout)),
                        span: None,
                    });
                }
                if copied.storage == nyar_types::layout::StorageKind::Value {
                    merged.value_type_declarations.insert(item);
                }
            }
            else if let Some(key) = layout.key.clone() {
                if merged.synthetic_layouts.insert(key, merged_id).is_some() {
                    return Err(ParseError::Invalid { message: format!("布局键 `{:?}` 重复", layout.key), span: None });
                }
            }
            merged.layouts.push(copied);
        }
    }
    Ok(merged)
}

fn declaration_for_layout(module: &MirModule, layout: &AggregateLayout) -> Option<nyar_types::ItemId> {
    module.aggregate_layouts.declaration_to_layout.iter().find_map(|(item, id)| (*id == layout.id).then_some(*item))
}

fn layout_contract_differs(existing: &AggregateLayout, incoming: &AggregateLayout) -> bool {
    existing.storage != incoming.storage
        || existing.size != incoming.size
        || existing.align != incoming.align
        || existing.fields != incoming.fields
}

fn layout_diagnostic_label(layout: &AggregateLayout) -> String {
    if layout.namespace.is_empty() { layout.name.clone() } else { format!("{}.{}", layout.namespace, layout.name) }
}

/// 把依赖模块本地 `LayoutId` 重写为合并后的布局身份。
///
/// 布局 id 只在单个模块的 plan 内稠密有效，不是跨模块身份。链接后字段里的
/// `Nominal { layout }` 必须指向合并 plan 的身份：否则同一声明会因两边本地
/// 编号不同被误判成合同冲突，后端也会按错误身份解析嵌套字段。
fn normalize_layout_references(
    layout: &AggregateLayout,
    local_to_merged: &BTreeMap<LayoutId, LayoutId>,
) -> Result<AggregateLayout, ParseError> {
    let mut normalized = layout.clone();
    let label = layout_diagnostic_label(layout);
    for field in &mut normalized.fields {
        field.ty = remap_layout_references(&field.ty, local_to_merged, &label)?;
    }
    Ok(normalized)
}

fn remap_layout_references(
    ty: &NyarType,
    local_to_merged: &BTreeMap<LayoutId, LayoutId>,
    label: &str,
) -> Result<NyarType, ParseError> {
    let missing = |layout: LayoutId| ParseError::Invalid {
        message: format!("布局 `{label}` 字段类型引用未登记布局 #{layout}"),
        span: None,
    };
    Ok(match ty {
        NyarType::Nominal { layout, spelling } => NyarType::Nominal {
            layout: *local_to_merged.get(layout).ok_or_else(|| missing(*layout))?,
            spelling: spelling.clone(),
        },
        NyarType::Apply(base, arguments) => NyarType::Apply(
            Box::new(remap_layout_references(base, local_to_merged, label)?),
            arguments
                .iter()
                .map(|argument| remap_layout_references(argument, local_to_merged, label))
                .collect::<Result<_, _>>()?,
        ),
        NyarType::Function(function) => NyarType::Function(Box::new(NyarFunctionType {
            params: function
                .params
                .iter()
                .map(|param| remap_layout_references(param, local_to_merged, label))
                .collect::<Result<_, _>>()?,
            return_type: remap_layout_references(&function.return_type, local_to_merged, label)?,
        })),
        NyarType::Tuple(items) => NyarType::Tuple(
            items.iter().map(|item| remap_layout_references(item, local_to_merged, label)).collect::<Result<_, _>>()?,
        ),
        NyarType::Array(element) => NyarType::Array(Box::new(remap_layout_references(element, local_to_merged, label)?)),
        NyarType::FixedArray { element, length } => NyarType::FixedArray {
            element: Box::new(remap_layout_references(element, local_to_merged, label)?),
            length: *length,
        },
        NyarType::TraitObject(witness) => NyarType::TraitObject(WitnessObject {
            trait_path: witness.trait_path.clone(),
            type_arguments: witness
                .type_arguments
                .iter()
                .map(|argument| remap_layout_references(argument, local_to_merged, label))
                .collect::<Result<_, _>>()?,
        }),
        NyarType::Nullable(inner) => NyarType::Nullable(Box::new(remap_layout_references(inner, local_to_merged, label)?)),
        NyarType::Union(items) => NyarType::Union(
            items.iter().map(|item| remap_layout_references(item, local_to_merged, label)).collect::<Result<_, _>>()?,
        ),
        other => other.clone(),
    })
}

#[derive(Default)]
struct AggregateRemap {
    nominals: BTreeMap<NominalInstanceId, NominalInstanceId>,
    variants: BTreeMap<nyar_types::VariantId, nyar_types::VariantId>,
    fields: BTreeMap<FieldId, FieldId>,
}

fn same_struct_contract(left: &MirStruct, right: &MirStruct) -> bool {
    left.declaration.is_some()
        && left.declaration == right.declaration
        && left.generics == right.generics
        && left.is_value_type == right.is_value_type
        && left.fields.iter().map(|field| (&field.name, &field.ty)).eq(right.fields.iter().map(|field| (&field.name, &field.ty)))
}

fn same_sum_contract(left: &MirSumDeclaration, right: &MirSumDeclaration) -> bool {
    left.declaration.is_some()
        && left.declaration == right.declaration
        && left.is_unite == right.is_unite
        && left.generics == right.generics
        && left
            .variants
            .iter()
            .map(|variant| {
                (
                    &variant.name,
                    variant.tag,
                    &variant.result_type,
                    variant.fields.iter().map(|field| (&field.name, &field.ty)).collect::<Vec<_>>(),
                )
            })
            .eq(right.variants.iter().map(|variant| {
                (
                    &variant.name,
                    variant.tag,
                    &variant.result_type,
                    variant.fields.iter().map(|field| (&field.name, &field.ty)).collect::<Vec<_>>(),
                )
            }))
}

fn freeze_aggregate_identities(
    consumer: &MirModule,
    dependencies: &[MirModule],
) -> Result<(Vec<MirStruct>, Vec<MirSumDeclaration>, Vec<AggregateRemap>), ParseError> {
    let modules = std::iter::once(consumer).chain(dependencies).collect::<Vec<_>>();
    let mut global_structs: Vec<MirStruct> = Vec::new();
    let mut global_sums: Vec<MirSumDeclaration> = Vec::new();
    let mut remaps = Vec::with_capacity(modules.len());
    // 字段身份只由 HIR 注册阶段生产；链接器只校验跨模块副本一致，不得重新编号。
    let mut field_owners: BTreeMap<FieldId, String> = BTreeMap::new();
    for module in modules {
        let mut remap = AggregateRemap::default();
        for declaration in &module.structs {
            let owner = struct_field_owner(declaration);
            if let Some(existing) =
                global_structs.iter().find(|existing| existing.declaration.is_some() && existing.declaration == declaration.declaration)
            {
                if !same_struct_contract(existing, declaration) {
                    return Err(ParseError::Invalid {
                        message: format!("聚合声明 `{}` 合同冲突", declaration.qualified_name()), span: None
                    });
                }
                remap.nominals.insert(declaration.nominal, existing.nominal);
                for (local, global) in declaration.fields.iter().zip(&existing.fields) {
                    if local.id != global.id {
                        return Err(ParseError::Invalid {
                            message: format!(
                                "聚合声明 `{}` 的字段身份不一致；local={}; global={}",
                                declaration.qualified_name(),
                                local.id,
                                global.id
                            ),
                            span: None,
                        });
                    }
                    claim_field_identity(&mut field_owners, local.id, &owner)?;
                    if remap.fields.insert(local.id, global.id).is_some() {
                        return Err(ParseError::Invalid {
                            message: format!("字段身份 `{}` 在模块 `{}` 中重复", local.id, module.name),
                            span: None,
                        });
                    }
                }
            }
            else {
                let nominal = NominalInstanceId::from_index(global_structs.len() as u32)
                    .ok_or_else(|| ParseError::Invalid { message: "NominalInstanceId 溢出".into(), span: None })?;
                let mut frozen = declaration.clone();
                frozen.nominal = nominal;
                remap.nominals.insert(declaration.nominal, nominal);
                for field in &frozen.fields {
                    claim_field_identity(&mut field_owners, field.id, &owner)?;
                    remap.fields.insert(field.id, field.id);
                }
                global_structs.push(frozen);
            }
        }
        for declaration in &module.sum_types {
            if let Some(existing) =
                global_sums.iter().find(|existing| existing.declaration.is_some() && existing.declaration == declaration.declaration)
            {
                if !same_sum_contract(existing, declaration) {
                    return Err(ParseError::Invalid { message: format!("sum 声明 `{}` 合同冲突", declaration.name), span: None });
                }
                remap.nominals.insert(declaration.nominal, existing.nominal);
                for (local_variant, global_variant) in declaration.variants.iter().zip(&existing.variants) {
                    let owner = sum_field_owner(declaration, local_variant);
                    remap.variants.insert(local_variant.id, global_variant.id);
                    for (local, global) in local_variant.fields.iter().zip(&global_variant.fields) {
                        if local.id != global.id {
                            return Err(ParseError::Invalid {
                                message: format!(
                                    "sum 声明 `{}` 变体 `{}` 的字段身份不一致；local={}; global={}",
                                    declaration.name, local_variant.name, local.id, global.id
                                ),
                                span: None,
                            });
                        }
                        claim_field_identity(&mut field_owners, local.id, &owner)?;
                        if remap.fields.insert(local.id, global.id).is_some() {
                            return Err(ParseError::Invalid {
                                message: format!("字段身份 `{}` 在模块 `{}` 中重复", local.id, module.name),
                                span: None,
                            });
                        }
                    }
                }
            }
            else {
                let mut frozen = declaration.clone();
                let global_nominal = NominalInstanceId::from_index((global_structs.len() + global_sums.len()) as u32)
                    .ok_or_else(|| ParseError::Invalid { message: "NominalInstanceId 溢出".into(), span: None })?;
                remap.nominals.insert(declaration.nominal, global_nominal);
                frozen.nominal = global_nominal;
                for (variant_index, variant) in frozen.variants.iter_mut().enumerate() {
                    let global_variant = nyar_types::VariantId::from_index(
                        global_sums.iter().flat_map(|sum| sum.variants.iter()).count() as u32 + variant_index as u32,
                    )
                    .ok_or_else(|| ParseError::Invalid { message: "VariantId 溢出".into(), span: None })?;
                    let owner = sum_field_owner(declaration, variant);
                    remap.variants.insert(variant.id, global_variant);
                    variant.id = global_variant;
                    for field in &variant.fields {
                        claim_field_identity(&mut field_owners, field.id, &owner)?;
                        remap.fields.insert(field.id, field.id);
                    }
                }
                global_sums.push(frozen);
            }
        }
        remaps.push(remap);
    }
    Ok((global_structs, global_sums, remaps))
}

fn struct_field_owner(declaration: &MirStruct) -> String {
    match declaration.declaration {
        Some(identity) => format!("struct#{identity}"),
        None => format!("anonymous:{}", declaration.qualified_name()),
    }
}

fn sum_field_owner(declaration: &MirSumDeclaration, variant: &crate::compiler::mir::MirSumVariant) -> String {
    match variant.declaration {
        Some(identity) => format!("variant#{identity}"),
        None => format!("sum:{}::{}", declaration.name, variant.name),
    }
}

fn claim_field_identity(owners: &mut BTreeMap<FieldId, String>, id: FieldId, owner: &str) -> Result<(), ParseError> {
    match owners.get(&id) {
        Some(existing) if existing == owner => Ok(()),
        Some(existing) => {
            Err(ParseError::Invalid { message: format!("字段身份 `{id}` 被 `{owner}` 与 `{existing}` 同时占用"), span: None })
        }
        None => {
            owners.insert(id, owner.to_string());
            Ok(())
        }
    }
}

/// 把将要安装进消费模块的依赖函数体副本，按其**来源模块**的 aggregate remap 归一。
///
/// provider 体在自身模块 lowering，`SumNew`/`SumPayloadGet`/`SumVariantIs`/`StructNew`
/// 携带的是来源模块的本地 `NominalInstanceId`/`VariantId`。安装进消费模块后，链接阶段
/// 只套消费模块自己的 remap；本地编号一旦不同就会指向另一个声明，表现为 SMIR006
/// 「undeclared variant」。安装前先用来源模块的重映射归一到消费模块已冻结的全局身份，
/// 再用同一组身份进入后续链接。
pub(super) fn remap_installed_provider_aggregates(
    consumer: &MirModule,
    dependencies: &[MirModule],
    source_module_index: usize,
    function: &mut MirFunction,
) -> Result<(), ParseError> {
    let (_, _, remaps) = freeze_aggregate_identities(consumer, dependencies)?;
    let remap = remaps.get(source_module_index).ok_or_else(|| ParseError::Invalid {
        message: "provider 来源模块身份不在当前源码闭包中".into(),
        span: None,
    })?;
    remap_function_aggregates(function, remap)
}

fn remap_function_aggregates(function: &mut MirFunction, remap: &AggregateRemap) -> Result<(), ParseError> {
    for block in &mut function.blocks {
        for instruction in &mut block.instructions {
            match &mut instruction.kind {
                MirOperation::StructNew { nominal, fields } => {
                    *nominal = *remap
                        .nominals
                        .get(nominal)
                        .ok_or_else(|| ParseError::Invalid { message: "StructNew 引用了未冻结的名义身份".into(), span: None })?;
                    for (field, _) in fields {
                        *field = *remap
                            .fields
                            .get(field)
                            .ok_or_else(|| ParseError::Invalid { message: "StructNew 引用了未冻结的字段身份".into(), span: None })?;
                    }
                }
                MirOperation::FieldGet { field, .. } | MirOperation::FieldSet { field, .. } => {
                    *field = *remap
                        .fields
                        .get(field)
                        .ok_or_else(|| ParseError::Invalid { message: "字段操作引用了未冻结的字段身份".into(), span: None })?;
                }
                MirOperation::SumNew { nominal, variant, .. }
                | MirOperation::SumPayloadGet { nominal, variant, .. }
                | MirOperation::SumVariantIs { nominal, variant, .. } => {
                    *nominal = *remap
                        .nominals
                        .get(nominal)
                        .ok_or_else(|| ParseError::Invalid { message: "sum 操作引用了未冻结的名义身份".into(), span: None })?;
                    *variant = *remap
                        .variants
                        .get(variant)
                        .ok_or_else(|| ParseError::Invalid { message: "sum 操作引用了未冻结的 variant 身份".into(), span: None })?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}
