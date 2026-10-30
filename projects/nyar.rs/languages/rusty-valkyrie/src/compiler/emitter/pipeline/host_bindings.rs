//! Resolver 已选定的 `[host_provider]` 绑定进入 Compiler linker 的合同。
//!
//! 装配层提交的是 inventory 声明身份；Compiler 在 HIR 物化后把它映射成冻结的
//! `ItemInstanceId`。Semantic MIR 只携带实例身份，不再保存声明索引 key。

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{ItemId, ItemInstanceId};

use crate::compiler::{
    emitter::backend_contract::interop::function_host_provider_target,
    mir::{MirExternalCallContract, MirFunction, MirModule, MirOperand, MirOperation},
    semantic::closure::{DeclarationKey, DeclarationKind},
    syntax::oak::ParseError,
    types::hir::{HirFunction, HirModule, ValkyrieType},
};

use super::context::CompilerHostProviderBinding;

/// 从依赖 MIR 中移除未选 `[host_provider]` 的外部导入合同（同文件多 provider 时仍只链入选中项）。
#[doc(hidden)]
pub fn prune_unselected_host_provider_external_calls(
    dependency_mirs: &mut [MirModule],
    consumer: &mut MirModule,
    hirs: &[&HirModule],
    host_bindings: &[CompilerHostProviderBinding],
) -> Result<(), ParseError> {
    let selected_providers = host_bindings.iter().map(|binding| binding.provider.clone()).collect::<BTreeSet<_>>();
    let prune_instances = unselected_host_provider_instances(hirs, &selected_providers)?;
    if prune_instances.is_empty() {
        return Ok(());
    }
    for module in dependency_mirs.iter_mut().chain(std::iter::once(consumer)) {
        module.external_calls.retain(|contract| match contract.instance {
            Some(instance) => !prune_instances.contains(&instance),
            None => true,
        });
    }
    Ok(())
}

fn unselected_host_provider_instances(
    hirs: &[&HirModule],
    selected_providers: &BTreeSet<DeclarationKey>,
) -> Result<BTreeSet<ItemInstanceId>, ParseError> {
    let mut prune = BTreeSet::new();
    for module in hirs {
        for function in &module.functions {
            if function_host_provider_target(function).is_none() {
                continue;
            }
            let key =
                declaration_key(module, DeclarationKind::Micro, &namespace_parts(&function.declaring_namespace), None, function.name.as_str());
            // 精确匹配或忽略 source_path 的 fallback
            let is_selected = selected_providers.contains(&key) || selected_providers.iter().any(|sel| {
                sel.dependency_key == key.dependency_key
                    && sel.namespace == key.namespace
                    && sel.kind == key.kind
                    && sel.owner == key.owner
                    && sel.name == key.name
            });
            if is_selected {
                continue;
            }
            let Some(instance) = function.instance
            else {
                continue;
            };
            prune.insert(instance);
        }
    }
    Ok(prune)
}

/// Resolver 选定且已映射为 Compiler 实例身份的 host provider 绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedHostProviderBinding {
    /// `[host_contract]` 在源码闭包中的 callable 实例族：声明实例加上单态化按
    /// 同一声明身份铸造的全部实例化副本。
    ///
    /// impl 成员的调用点身份是铸造实例而非声明实例；绑定若只覆盖声明实例，
    /// 合同体会以实例化孤儿的形态残留在产物里。
    pub contract_instances: BTreeSet<ItemInstanceId>,
    /// 选定 `[host_provider]` 实现的 callable 实例。
    pub provider_instance: ItemInstanceId,
}

struct PlannedBinding {
    contract_instances: BTreeSet<ItemInstanceId>,
    provider_instance: ItemInstanceId,
    provider_function: Option<MirFunction>,
}

/// 把 Resolver 提交的声明身份映射为 Compiler 冻结的 callable 实例。
///
/// 未被源码闭包引用的合同不产生绑定；provider 身份缺失即结构化失败。
#[doc(hidden)]
pub fn resolve_host_provider_bindings(
    hirs: &[&HirModule],
    bindings: &[CompilerHostProviderBinding],
) -> Result<Vec<ResolvedHostProviderBinding>, ParseError> {
    if bindings.is_empty() {
        return Ok(Vec::new());
    }
    let instances = host_callable_instances(hirs)?;
    let mut resolved = Vec::new();
    for binding in bindings {
        // 精确匹配；未命中则忽略 source_path fallback（planner 按文件构造，HIR 按 group 合并）。
        let contract_instances = match instances.get(&binding.contract) {
            Some(family) => family.clone(),
            None => {
                let mut union = BTreeSet::new();
                for (key, family) in &instances {
                    if key.dependency_key == binding.contract.dependency_key
                        && key.namespace == binding.contract.namespace
                        && key.kind == binding.contract.kind
                        && key.owner == binding.contract.owner
                        && key.name == binding.contract.name
                    {
                        union.extend(family.iter().copied());
                    }
                }
                union
            }
        };
        if contract_instances.is_empty() {
            continue;
        }
        let provider_family = instances.get(&binding.provider).or_else(|| {
            instances.iter().find(|(key, _)| {
                key.dependency_key == binding.provider.dependency_key
                    && key.namespace == binding.provider.namespace
                    && key.kind == binding.provider.kind
                    && key.owner == binding.provider.owner
                    && key.name == binding.provider.name
            }).map(|(_, family)| family)
        });
        let Some(provider_family) = provider_family else {
            continue;
        };
        if provider_family.len() != 1 {
            return Err(ParseError::Invalid {
                message: format!("host provider 声明 `{}` 对应 {} 个实例", binding.provider.encoded(), provider_family.len()),
                span: None,
            });
        }
        let provider_instance = *provider_family.iter().next().expect("single provider instance");
        resolved.push(ResolvedHostProviderBinding { contract_instances, provider_instance });
    }
    Ok(resolved)
}

/// HIR 已注册 callable 身份 → inventory 声明身份的映射（声明身份 → 实例族）。
///
/// 映射只消费 HIR 已注册的 `declaration` / `instance`；声明种类与拥有者必须与语义闭包
/// `collect_item` / `collect_members` / `collect_imply` 的登记一致，否则成员合同
/// 会被当成“未进入闭包”而静默跳过。声明侧同一名字出现多个声明身份或多个声明实例
/// 按结构化失败处理；单态化铸造副本（`instantiated_callables`）不参与名字扫描，
/// 按同一声明 `ItemId` 挂到实例族下——调用点只指向铸造实例，合同必须覆盖整个族。
fn host_callable_instances(hirs: &[&HirModule]) -> Result<BTreeMap<DeclarationKey, BTreeSet<ItemInstanceId>>, ParseError> {
    let mut collected = BTreeMap::<DeclarationKey, CallableFamily>::new();
    for module in hirs {
        for function in &module.functions {
            collect_callable_instance(
                &mut collected,
                declaration_key(module, DeclarationKind::Micro, &namespace_parts(&function.declaring_namespace), None, function.name.as_str()),
                function,
            );
        }
        for structure in &module.structs {
            let namespace: Vec<String> = structure.namespace.iter().map(|part| part.as_str().to_string()).collect();
            for method in &structure.methods {
                collect_callable_instance(
                    &mut collected,
                    declaration_key(module, DeclarationKind::Method, &namespace, Some(structure.name.as_str()), method.name.as_str()),
                    method,
                );
            }
        }
        for trait_decl in &module.traits {
            let namespace = namespace_parts(&trait_decl.declaring_namespace);
            for method in trait_decl.methods.iter().chain(trait_decl.default_methods.iter()) {
                collect_callable_instance(
                    &mut collected,
                    declaration_key(module, DeclarationKind::Method, &namespace, Some(trait_decl.name.as_str()), method.name.as_str()),
                    method,
                );
            }
        }
        for implementation in &module.impls {
            let Some(owner) = target_owner_name(module, &implementation.target)
            else {
                continue;
            };
            let namespace = namespace_parts(&implementation.declaring_namespace);
            for method in &implementation.methods {
                collect_callable_instance(
                    &mut collected,
                    declaration_key(module, DeclarationKind::ImplyMethod, &namespace, Some(owner), method.name.as_str()),
                    method,
                );
            }
        }
        for export in &module.imported_semantic_exports {
            let module_name = export.module.to_string();
            for function in &export.functions {
                collect_callable_instance(
                    &mut collected,
                    DeclarationKey {
                        dependency_key: module_name.clone(),
                        source_path: module_name.clone(),
                        namespace: namespace_parts(&function.declaring_namespace),
                        kind: DeclarationKind::Micro,
                        owner: None,
                        name: function.name.as_str().to_string(),
                    },
                    function,
                );
            }
        }
    }
    let mut instances = BTreeMap::new();
    let mut mint_owner = BTreeMap::<ItemId, DeclarationKey>::new();
    for (key, family) in collected {
        if family.declarations.len() > 1 || family.instances.len() > 1 {
            return Err(ParseError::Invalid {
                message: format!("callable 声明 `{}` 对应多个声明身份或实例", key.encoded()), span: None
            });
        }
        for declaration in &family.declarations {
            mint_owner.insert(*declaration, key.clone());
        }
        instances.insert(key, family.instances);
    }
    for module in hirs {
        for callable in &module.instantiated_callables {
            let (Some(declaration), Some(instance)) = (callable.function.declaration, callable.function.instance) else {
                continue;
            };
            let Some(key) = mint_owner.get(&declaration) else {
                continue;
            };
            if let Some(family) = instances.get_mut(key) {
                family.insert(instance);
            }
        }
    }
    Ok(instances)
}

/// 声明身份 → 实例族条目：声明侧实例与单态化铸造副本（后续按 `ItemId` 汇入）合并。
#[derive(Default)]
struct CallableFamily {
    declarations: BTreeSet<ItemId>,
    instances: BTreeSet<ItemInstanceId>,
}

fn collect_callable_instance(collected: &mut BTreeMap<DeclarationKey, CallableFamily>, key: DeclarationKey, function: &HirFunction) {
    let family = collected.entry(key).or_default();
    if let Some(declaration) = function.declaration {
        family.declarations.insert(declaration);
    }
    if let Some(instance) = function.instance {
        family.instances.insert(instance);
    }
}

fn declaration_key(module: &HirModule, kind: DeclarationKind, namespace: &[String], owner: Option<&str>, name: &str) -> DeclarationKey {
    DeclarationKey {
        dependency_key: module.dependency_key.clone(),
        source_path: module.source_path.clone(),
        namespace: namespace.to_vec(),
        kind,
        owner: owner.map(str::to_string),
        name: name.to_string(),
    }
}

fn namespace_parts(namespace: &crate::compiler::types::NamePath) -> Vec<String> {
    namespace.parts().iter().map(|part| part.as_str().to_string()).collect()
}

/// `imply` 目标类型的聚合短名：与闭包 `imply_owner_name` 保持同一事实来源。
///
/// 声明绑定阶段会把可解析的目标冻结为 `Nominal { declaration }`（`bind_declared_nominal_types`），
/// 短名不再留在类型表达式里；此时按冻结的 `ItemId` 从模块本地声明表与导入语义导出中恢复
/// 声明短名（与 HIR `impl_self_owner` / `lookup_owner_generics_by_declaration` 同一查找事实源）。
/// 未登记的 `ItemId` 返回 None，不做按名猜测。
fn target_owner_name<'a>(module: &'a HirModule, ty: &'a ValkyrieType) -> Option<&'a str> {
    match ty {
        ValkyrieType::Named(name) => Some(name.as_str()),
        ValkyrieType::Apply(base, _) => target_owner_name(module, base),
        ValkyrieType::Nominal { declaration, .. } => declared_owner_name(module, *declaration),
        // 停线清理：QualifiedPath 不得在 host_bindings 阶段参与 owner 猜测
        // （短名 fallback 违反架构合同）。无显式绑定则返回 None。
        _ => None,
    }
}

/// 冻结声明身份 → 聚合声明短名：本地 structs/enums 优先，其次导入语义导出。
fn declared_owner_name(module: &HirModule, declaration: nyar_types::ItemId) -> Option<&str> {
    if let Some(structure) = module.structs.iter().find(|item| item.constructor_declaration == Some(declaration)) {
        return Some(structure.name.as_str());
    }
    if let Some(enum_def) = module.enums.iter().find(|item| item.declaration == Some(declaration)) {
        return Some(enum_def.name.as_str());
    }
    for export in &module.imported_semantic_exports {
        if let Some(structure) = export.structs.iter().find(|item| item.constructor_declaration == Some(declaration)) {
            return Some(structure.name.as_str());
        }
        if let Some(enum_def) = export.enums.iter().find(|item| item.declaration == Some(declaration)) {
            return Some(enum_def.name.as_str());
        }
    }
    None
}

/// 将 Resolver 选定的 host provider 绑定应用到已链接的 Semantic MIR，
/// 并在编译边界执行「已解析 callee」的 fail-closed 门禁。
#[doc(hidden)]
pub fn apply_host_provider_bindings(
    module: &mut MirModule,
    dependency_mirs: &[MirModule],
    bindings: &[ResolvedHostProviderBinding],
) -> Result<(), ParseError> {
    if !bindings.is_empty() {
        remap_bound_host_contracts(module, dependency_mirs, bindings)?;
    }
    enforce_resolved_host_contract_bindings(module)
}

/// 编译边界门禁：已解析调用指向未绑定 provider 的 `[host_contract]` 即结构化失败。
///
/// 判据是 linked consumer 中的真实 callee，而不是源码闭包的保守超集：
/// 空 host 链接（`boundary == host` 且无平台标签、无定位片段）是无体合同
/// 降低后的唯一链接形态；provider 重映射完成后仍有 Call 指向该合同实例，
/// 说明实现来源缺失，禁止静默编码成空体或默认值。
/// 闭包提升但从未被可达代码调用的合同不在此失败——链接 BFS 不会把
/// 不可达成员与合同并入 consumer。
fn enforce_resolved_host_contract_bindings(module: &MirModule) -> Result<(), ParseError> {
    let unbound: BTreeMap<ItemInstanceId, &MirExternalCallContract> = module
        .external_calls
        .iter()
        .filter(|contract| {
            contract.link.matches_boundary("host") && contract.link.platform_tag.is_none() && contract.link.locator_segments.is_empty()
        })
        .filter_map(|contract| contract.instance.map(|instance| (instance, contract)))
        .collect();
    if unbound.is_empty() {
        return Ok(());
    }
    for function in &module.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let MirOperation::Call { callee, arguments } = &instruction.kind {
                    for operand in std::iter::once(callee).chain(arguments.iter()) {
                        if let MirOperand::Callable(instance) = operand
                            && let Some(contract) = unbound.get(instance)
                        {
                            return Err(ParseError::Invalid {
                                message: format!(
                                    "host contract `{}` (instance `{instance}`) has a resolved call but no host provider is bound",
                                    contract.symbol
                                ),
                                span: None,
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// 按 Resolver 绑定把合同调用重映射到 provider，并安装 provider 定义/导入。
///
/// 合同身份覆盖整个实例族：族内每个实例的调用都重定向到 provider，族内全部
/// 合同体（声明体与实例化副本）一并退役，不得留下进入后端的孤儿。
fn remap_bound_host_contracts(
    module: &mut MirModule,
    dependency_mirs: &[MirModule],
    bindings: &[ResolvedHostProviderBinding],
) -> Result<(), ParseError> {
    let modules: Vec<&MirModule> = std::iter::once(&*module).chain(dependency_mirs.iter()).collect();
    let mut plans = Vec::new();

    for binding in bindings {
        if !binding.contract_instances.iter().any(|instance| callable_present(&modules, *instance)) {
            continue;
        }
        if !callable_present(&modules, binding.provider_instance) {
            return Err(ParseError::Invalid {
                message: format!("host provider `{}` 未在源码闭包中定义", binding.provider_instance),
                span: None,
            });
        }
        let provider_function = modules
            .iter()
            .flat_map(|dependency| dependency.functions.iter())
            .find(|function| function.instance == Some(binding.provider_instance))
            .cloned();
        plans.push(PlannedBinding {
            contract_instances: binding.contract_instances.clone(),
            provider_instance: binding.provider_instance,
            provider_function,
        });
    }

    // 先完成全部合同的身份映射校验，再安装 provider 定义；冲突绑定不得留下部分副作用。
    let mut remaps = BTreeMap::<ItemInstanceId, ItemInstanceId>::new();
    for plan in &plans {
        for contract_instance in &plan.contract_instances {
            if *contract_instance == plan.provider_instance {
                continue;
            }
            if let Some(previous) = remaps.insert(*contract_instance, plan.provider_instance)
                && previous != plan.provider_instance
            {
                return Err(ParseError::Invalid { message: "host contract 不能同时绑定到多个 provider".into(), span: None });
            }
        }
    }

    // provider 的外部导入合同：无体 provider 由目标指令实现，其链接事实在依赖模块的
    // external_calls 里。重映射会移除原合同导入，必须把 provider 自己的导入合同
    // 一并迁入消费模块，否则调用既无定义也无导入。
    let selected_provider_instances = plans.iter().map(|plan| plan.provider_instance).collect::<BTreeSet<_>>();
    let provider_imports = modules
        .iter()
        .flat_map(|dependency| dependency.external_calls.iter())
        .filter(|contract| contract.instance.is_some())
        .filter(|contract| selected_provider_instances.contains(&contract.instance.expect("provider import instance")))
        .map(|contract| (contract.instance.expect("provider import instance"), contract.clone()))
        .collect::<BTreeMap<ItemInstanceId, MirExternalCallContract>>();

    for plan in plans {
        if let Some(provider_function) = plan.provider_function {
            install_provider_definition(module, plan.provider_instance, provider_function)?;
        }
        else if let Some(contract) = provider_imports.get(&plan.provider_instance) {
            install_provider_import(module, contract)?;
        }
    }

    if remaps.is_empty() {
        return Ok(());
    }

    for function in &mut module.functions {
        remap_call_targets(function, &remaps);
    }

    for contract_instance in remaps.keys() {
        module.callable_symbols.remove(contract_instance);
        module.external_calls.retain(|contract| contract.instance != Some(*contract_instance));
        // 合同体退役（含实例化副本）：调用已全部重映射到 provider 后，合同实现不再
        // 有任何调用身份；链接 BFS 在重映射之前已把合同体与铸造副本拉入 linked 模块，
        // 若不在此移除，它们将作为孤儿定义留在模块中并进入后端校验（其聚合字段指令
        // 与 provider 表示不一致即被拒绝）。
        module.functions.retain(|function| function.instance != Some(*contract_instance));
    }

    Ok(())
}

fn callable_present(modules: &[&MirModule], instance: ItemInstanceId) -> bool {
    modules.iter().any(|module| {
        module.functions.iter().any(|function| function.instance == Some(instance))
            || module.external_calls.iter().any(|contract| contract.instance == Some(instance))
    })
}

fn install_provider_definition(
    module: &mut MirModule,
    provider_instance: ItemInstanceId,
    provider_function: MirFunction,
) -> Result<(), ParseError> {
    if module.functions.iter().any(|function| function.instance == Some(provider_instance)) {
        let symbol = format!("{}#{}", provider_function.display_symbol, provider_instance.index());
        module
            .register_callable_symbol(provider_instance, symbol)
            .map_err(|e| ParseError::Invalid { message: e, span: None })?;
        return Ok(());
    }
    module.functions.push(provider_function.clone());
    let symbol = format!("{}#{}", provider_function.display_symbol, provider_instance.index());
    module
        .register_callable_symbol(provider_instance, symbol)
        .map_err(|e| ParseError::Invalid { message: e, span: None })?;
    Ok(())
}

/// 把无体 provider 的外部导入合同迁入消费模块。
///
/// provider 没有函数体时，其目标绑定（`[wasm_operator]` / `[ovm_operator]` 等）只以外部导入
/// 合同存在；重映射移除原合同后，消费模块必须持有 provider 自己的合同身份，
/// 后端才能按已绑定的目标编码调用。
fn install_provider_import(module: &mut MirModule, contract: &MirExternalCallContract) -> Result<(), ParseError> {
    let instance = contract.instance.expect("provider import instance");
    if !module.external_calls.contains(contract) {
        module.external_calls.push(contract.clone());
    }
    let symbol = crate::compiler::hir::symbols::stable_symbol_key(&contract.symbol);
    let symbol = format!("{symbol}#{}", instance.index());
    module.register_callable_symbol(instance, symbol).map_err(|detail| ParseError::Invalid { message: detail, span: None })
}

fn remap_call_targets(function: &mut MirFunction, remaps: &BTreeMap<ItemInstanceId, ItemInstanceId>) {
    for block in &mut function.blocks {
        for instruction in &mut block.instructions {
            if let MirOperation::Call { callee, arguments } = &mut instruction.kind {
                if let MirOperand::Callable(instance) = callee {
                    if let Some(target) = remaps.get(instance) {
                        *callee = MirOperand::Callable(*target);
                    }
                }
                for argument in arguments {
                    if let MirOperand::Callable(instance) = argument {
                        if let Some(target) = remaps.get(instance) {
                            *argument = MirOperand::Callable(*target);
                        }
                    }
                }
            }
        }
    }
}
