//! Semantic MIR 的实例身份链接边界。
//!
//! 链接器只接受前端已经冻结的 ItemInstanceId 闭包，不从名称或布局补造身份。

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::{
    hir::overload::has_unresolved_generic_arguments,
    mir::{MirFunction, MirModule, MirOperand, MirOperation, MirStruct, MirSumDeclaration},
    syntax::oak::ParseError,
    types::hir::{export_spec::parse_library_glue_operation, HirModule, ValkyrieType},
};
use nyar::planning::SemanticAggregateOperation;
use nyar_types::{
    FieldId, ItemId, ItemInstanceId, LibraryGlueOperation, NominalInstanceId,
    layout::{AggregateLayout, AggregateLayoutPlan},
};

use super::host_bindings::remap_call_targets;

/// 合并已由 Compiler 统一注册的依赖实例闭包。
///
/// 不携带 HIR 闭包时无法看到 `[library_glue]` 角色事实，聚合直通操作阶段不运行；
/// 正常编译路径使用 [`link_reachable_dependency_mir_with_hir`]。
#[doc(hidden)]
pub fn link_reachable_dependency_mir(
    consumer: &mut MirModule,
    dependency_mirs: &[MirModule],
) -> Result<(), crate::compiler::syntax::oak::ParseError> {
    link_reachable_dependency_mir_with_hir(consumer, dependency_mirs, &[], &BTreeMap::new())
}

/// 合并依赖实例闭包，并把片段根签名所需的聚合直通操作冻结进 Semantic MIR。
///
/// `host_binding_remaps` 是本次编译中 `apply_host_provider_bindings` 已实际执行的
/// 合同→provider 调用改写表；首轮链接传空表。绑定会把消费模块内全部函数（含首轮
/// 链接迁入的依赖体副本）的 Call 目标改写为 provider 身份，二次链接据此把依赖原件
/// 规范化到同一绑定后状态，再与消费模块副本逐字段核对；规范化后仍不一致即真实冲突。
#[doc(hidden)]
pub fn link_reachable_dependency_mir_with_hir(
    consumer: &mut MirModule,
    dependency_mirs: &[MirModule],
    hirs: &[&HirModule],
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
                        else if host_binding_remaps.is_empty() {
                            false
                        } else {
                            let mut normalized = (*function).clone();
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
    // 片段根签名需要的聚合直通操作必须先冻结，并把生产者加入可达根，
    // 否则这些宿主直通成员既不会进入 linked，也无法在 canonical 找到生产者。
    freeze_fragment_aggregate_operations(hirs, &definitions, &mut linked, &mut pending)?;
    while let Some(instance) = pending.pop() {
        if !visited.insert(instance) {
            continue;
        }
        if let Some((module, function)) = definitions.get(&instance) {
            for block in &function.blocks {
                for instruction in &block.instructions {
                    if let MirOperation::Call { callee: MirOperand::Callable(callee), .. } = &instruction.kind {
                        pending.push(*callee);
                    }
                }
            }
            if !std::ptr::eq(*module, &*consumer) {
                let symbol = format!("{}#{}", function.display_symbol, instance.index());
                if let Err(detail) = linked.register_callable_symbol(instance, symbol) {
                    return Err(ParseError::Invalid { message: detail, span: None });
                }
                let module_index = std::iter::once(&*consumer)
                    .chain(dependency_mirs)
                    .position(|candidate| std::ptr::eq(candidate, *module))
                    .ok_or_else(|| ParseError::Invalid { message: "依赖模块身份不在当前源码闭包中".into(), span: None })?;
                let mut function = (*function).clone();
                remap_function_aggregates(&mut function, &remaps[module_index])?;
                linked.functions.push(function);
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

/// 冻结片段根签名需要的聚合直通操作，并把生产者加入可达性根。
///
/// 角色只来自 HIR 声明的 `[library_glue]` 标注，生产者只来自已实例化、
/// 且 owner 类型与片段根签名完全具体名义实例一致的 callable。找不到生产者
/// 时失败闭合，不按成员名、布局或声明级身份猜映射。
fn freeze_fragment_aggregate_operations(
    hirs: &[&HirModule],
    definitions: &BTreeMap<ItemInstanceId, (&MirModule, &MirFunction)>,
    linked: &mut MirModule,
    pending: &mut Vec<ItemInstanceId>,
) -> Result<(), ParseError> {
    let (glue_roles, producers) = collect_library_glue_facts(hirs)?;
    if glue_roles.is_empty() {
        return Ok(());
    }
    for fragment in &mut linked.semantic_fragments {
        // 无导出根的片段不承载聚合直通操作。
        if fragment.exported_operations.is_empty() {
            continue;
        }
        let mut concretes = BTreeSet::new();
        for instance in &fragment.exported_operations {
            let Some((_, function)) = definitions.get(instance)
            else {
                return Err(ParseError::Invalid {
                    message: format!("片段 `{}` 的片段根实例 `{instance}` 缺少函数体，无法冻结聚合直通操作", fragment.id),
                    span: None,
                });
            };
            for parameter in &function.param_types {
                collect_concrete_nominal_types(parameter, &mut concretes);
            }
            collect_concrete_nominal_types(&function.return_type, &mut concretes);
        }
        let mut required = Vec::new();
        for concrete in &concretes {
            let ValkyrieType::Nominal { declaration, .. } = concrete
            else {
                continue;
            };
            let Some(roles) = glue_roles.get(declaration)
            else {
                continue;
            };
            let found = producers.get(concrete);
            for role in roles {
                let Some(instance) = found.and_then(|operations| operations.get(role)).copied()
                else {
                    return Err(ParseError::Invalid {
                        message: format!(
                            "片段 `{}` 的根签名需要聚合 `{concrete:?}` 的 `[library_glue({})]` 生产者，但闭包内没有对应的已实例化成员（声明缺 `[library_glue]` 角色成员或未实例化）",
                            fragment.id,
                            role.as_str()
                        ),
                        span: None,
                    });
                };
                required.push((concrete.clone(), *role, instance));
            }
        }
        // 同一 (type_id, operation) 只冻结一条；链接可能因宿主绑定后补闭包而重跑，
        // 已冻结的操作必须视为已存在，不得重复追加。
        let mut seen = fragment.aggregate_operations.iter().map(|operation| (operation.type_id, operation.operation)).collect::<BTreeSet<_>>();
        for (concrete, operation, instance) in required {
            let type_id = linked.type_identities.get(&concrete).copied().ok_or_else(|| ParseError::Invalid {
                message: format!("片段 `{}` 的聚合实例 `{concrete:?}` 不在 Compiler 统一 TypeId 表中", fragment.id),
                span: None,
            })?;
            // 同一 (type_id, operation) 只冻结一条；同一片段内不重复。
            if !seen.insert((type_id, operation)) {
                continue;
            }
            fragment.aggregate_operations.push(SemanticAggregateOperation { type_id, operation, instance });
            pending.push(instance);
        }
    }
    Ok(())
}

/// 收集 HIR 闭包里的 `[library_glue]` 角色集合与已实例化的生产者。
///
/// 角色集合以名义声明身份为键，用于判断片段根签名「需要」哪些直通操作；
/// 生产者以完全具体的 owner 类型为键，值必须是唯一的已实例化 callable。
fn collect_library_glue_facts(
    hirs: &[&HirModule],
) -> Result<
    (BTreeMap<ItemId, BTreeSet<LibraryGlueOperation>>, BTreeMap<ValkyrieType, BTreeMap<LibraryGlueOperation, ItemInstanceId>>),
    ParseError,
> {
    let mut roles = BTreeMap::new();
    let mut producers = BTreeMap::new();
    for hir in hirs {
        collect_library_glue_facts_from_module(hir, &mut roles, &mut producers)?;
    }
    Ok((roles, producers))
}

fn collect_library_glue_facts_from_module(
    module: &HirModule,
    roles: &mut BTreeMap<ItemId, BTreeSet<LibraryGlueOperation>>,
    producers: &mut BTreeMap<ValkyrieType, BTreeMap<LibraryGlueOperation, ItemInstanceId>>,
) -> Result<(), ParseError> {
    for implementation in &module.impls {
        let declaration = implementation.target.nominal_declaration();
        for method in &implementation.methods {
            let Some(operation) = parse_library_glue_operation(&method.annotations)
            else {
                continue;
            };
            let Some(declaration) = declaration
            else {
                return Err(ParseError::Invalid {
                    message: format!("聚合 `{:?}` 的 `[library_glue({})]` 成员缺少名义声明身份", implementation.target, operation.as_str()),
                    span: None,
                });
            };
            roles.entry(declaration).or_default().insert(operation);
        }
    }
    for callable in &module.instantiated_callables {
        let Some(operation) = parse_library_glue_operation(&callable.function.annotations)
        else {
            continue;
        };
        let Some(instance) = callable.function.instance
        else {
            continue;
        };
        let Some(owner) = callable.owner_type.as_ref()
        else {
            continue;
        };
        // 只有完全具体的名义实例才能与片段根签名按类型身份对上。
        let ValkyrieType::Nominal { arguments, .. } = owner
        else {
            continue;
        };
        if arguments.is_empty() || has_unresolved_generic_arguments(owner) {
            continue;
        }
        if let Some(previous) = producers.entry(owner.clone()).or_default().insert(operation, instance)
            && previous != instance
        {
            return Err(ParseError::Invalid {
                message: format!(
                    "聚合实例 `{owner:?}` 的 `[library_glue({})]` 角色绑定多个生产者: {previous} / {instance}",
                    operation.as_str()
                ),
                span: None,
            });
        }
    }
    for submodule in &module.submodules {
        collect_library_glue_facts_from_module(submodule, roles, producers)?;
    }
    Ok(())
}

/// 递归收集类型里完全具体的名义实例（含嵌套实参）；仍含开放 binder 的形状跳过。
fn collect_concrete_nominal_types(ty: &ValkyrieType, out: &mut BTreeSet<ValkyrieType>) {
    match ty {
        ValkyrieType::Nominal { arguments, .. } => {
            if !arguments.is_empty() && !has_unresolved_generic_arguments(ty) {
                out.insert(ty.clone());
            }
            for argument in arguments {
                collect_concrete_nominal_types(argument, out);
            }
        }
        ValkyrieType::Apply(base, arguments) => {
            collect_concrete_nominal_types(base, out);
            for argument in arguments {
                collect_concrete_nominal_types(argument, out);
            }
        }
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => collect_concrete_nominal_types(element, out),
        ValkyrieType::FixedArray { element, .. } => collect_concrete_nominal_types(element, out),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
            for item in items {
                collect_concrete_nominal_types(item, out);
            }
        }
        ValkyrieType::Function(function) => {
            for parameter in &function.params {
                collect_concrete_nominal_types(parameter, out);
            }
            collect_concrete_nominal_types(&function.return_type, out);
        }
        _ => {}
    }
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
        for layout in &module.aggregate_layouts.layouts {
            let owner = declaration_for_layout(module, layout);
            if let Some(item) = owner {
                if let Some(existing_id) = merged.declaration_to_layout.get(&item).copied()
                    && let Some(existing) = merged.layouts.iter().find(|candidate| candidate.id == existing_id)
                {
                    if layout_contract_differs(existing, layout) {
                        return Err(ParseError::Invalid {
                            message: format!("布局 `{}` 合同冲突", layout_diagnostic_label(layout)),
                            span: None,
                        });
                    }
                    continue;
                }
            }
            else if let Some(key) = layout.key.as_ref() {
                // 合成 tuple / fixedarray 以结构键认亲；键相同即同一物理形状。
                if let Some(existing_id) = merged.synthetic_layouts.get(key).copied() {
                    let existing = merged.layouts.iter().find(|candidate| candidate.id == existing_id);
                    if let Some(existing) = existing {
                        if layout_contract_differs(existing, layout) {
                            return Err(ParseError::Invalid { message: format!("布局键 `{key:?}` 合同冲突"), span: None });
                        }
                        continue;
                    }
                }
            }
            else {
                return Err(ParseError::Invalid {
                    message: format!("布局 `{}` 缺少 declaration identity，禁止按名合并", layout_diagnostic_label(layout)),
                    span: None,
                });
            }
            let new_id = merged.layouts.iter().map(|candidate| candidate.id).max().unwrap_or(0).saturating_add(1);
            let mut copied = layout.clone();
            copied.id = new_id;
            if let Some(item) = owner {
                // 只按声明身份合并；名称或物理形状相同不能补出语义身份。
                if merged.declaration_to_layout.insert(item, new_id).is_some() {
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
                if merged.synthetic_layouts.insert(key, new_id).is_some() {
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
