//! Resolver 已选定的 `[host_provider]` 绑定进入 Compiler linker 的合同。
//!
//! 装配层提交的是 inventory 声明身份；Compiler 在 HIR 物化后把它映射成冻结的
//! `ItemInstanceId`。Semantic MIR 只携带实例身份，不再保存声明索引 key。

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{ItemId, ItemInstanceId};

use crate::compiler::{
    emitter::backend_contract::interop::function_host_provider_target,
    hir::overload::has_unresolved_generic_arguments,
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
    let mut unselected_declarations = BTreeSet::new();
    for module in hirs {
        for function in &module.functions {
            if function_host_provider_target(function).is_none() {
                continue;
            }
            let key =
                declaration_key(module, DeclarationKind::Micro, &namespace_parts(&function.declaring_namespace), None, function.name.as_str());
            // 精确匹配或忽略 source_path 的 fallback
            let is_selected = selected_providers.contains(&key)
                || selected_providers.iter().any(|sel| {
                    sel.dependency_key == key.dependency_key
                        && sel.namespace == key.namespace
                        && sel.kind == key.kind
                        && sel.owner == key.owner
                        && sel.name == key.name
                });
            if is_selected {
                continue;
            }
            if let Some(declaration) = function.declaration {
                unselected_declarations.insert(declaration);
            }
            if let Some(instance) = function.instance {
                prune.insert(instance);
            }
        }
        // 泛型 provider 的单态化铸造副本同属未选定 provider 族：按声明身份裁剪，
        // 否则播种出的实现副本会以孤儿形态进入产物。
        for callable in &module.instantiated_callables {
            if callable.function.declaration.is_some_and(|declaration| unselected_declarations.contains(&declaration))
                && let Some(instance) = callable.function.instance
            {
                prune.insert(instance);
            }
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
    /// 闭合合同实例 → 同签名闭合 provider 实例的配对表。
    ///
    /// 泛型 provider 的声明级实例签名开放，只能承接同样开放的调用形态；绑定
    /// 把闭合合同副本配到单态化铸造出的同签名 provider 副本上，实例身份、
    /// 签名与实现全部来自声明与 provider 本身，不存在按名回落或开放实例兜底。
    /// 开放合同实例不配对，仅随族退役。
    pub provider_pairs: BTreeMap<ItemInstanceId, ItemInstanceId>,
}

struct PlannedBinding {
    contract_instances: BTreeSet<ItemInstanceId>,
    pairs: BTreeMap<ItemInstanceId, ItemInstanceId>,
    provider_functions: Vec<(ItemInstanceId, MirFunction)>,
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
        let contract_family = family_by_binding_key(&instances, &binding.contract);
        if contract_family.instances.is_empty() {
            continue;
        }
        // 绑定键必须落在单一已解析声明身份上：编码名下聚合多个声明身份时，
        // Resolver 的名字键无法区分重载，属真歧义，结构化拒绝；同一声明身份族内
        // 的多个实例（声明级实例 + 单态化铸造副本）合法，全部各自绑定。
        if contract_family.declarations.len() > 1 {
            return Err(ParseError::Invalid {
                message: format!("callable 声明 `{}` 对应多个声明身份或实例", binding.contract.encoded()),
                span: None,
            });
        }
        let provider_family = family_by_binding_key(&instances, &binding.provider);
        if provider_family.instances.is_empty() {
            continue;
        }
        // provider 配对按具体签名逐项进行：泛型 provider 的声明级实例签名开放，
        // 只有与闭合合同副本逐项同签名的单态化闭合副本能承接调用；闭合合同实例
        // 找不到同签名 provider 实例即结构化失败，禁止把调用重映射回开放声明级
        // 实例（其体内宿主导入从未闭合）。开放合同实例不配对，只随族退役。
        let mut provider_pairs = BTreeMap::new();
        for contract_instance in &contract_family.instances {
            let Some(signature) = contract_family.signatures.get(contract_instance) else {
                continue;
            };
            if host_signature_is_open(signature) {
                continue;
            }
            let mut matched: Option<ItemInstanceId> = None;
            for provider_instance in &provider_family.instances {
                let Some(provider_signature) = provider_family.signatures.get(provider_instance) else {
                    continue;
                };
                if provider_signature != signature {
                    continue;
                }
                if matched.is_some() {
                    return Err(ParseError::Invalid {
                        message: format!("host provider 声明 `{}` 存在多个与合同实例 `{contract_instance}` 同签名的实例", binding.provider.encoded()),
                        span: None,
                    });
                }
                matched = Some(*provider_instance);
            }
            let Some(provider_instance) = matched else {
                return Err(ParseError::Invalid {
                    message: format!(
                        "host provider 声明 `{}` 没有与闭合合同实例 `{contract_instance}` 同签名的 provider 实例（provider 声明为泛型时，其具体代入由合同调用点在单态化播种中铸造）",
                        binding.provider.encoded()
                    ),
                    span: None,
                });
            };
            provider_pairs.insert(*contract_instance, provider_instance);
        }
        resolved.push(ResolvedHostProviderBinding { contract_instances: contract_family.instances, provider_pairs });
    }
    Ok(resolved)
}

/// 按绑定声明键取出实例族：精确命中优先；未命中则忽略 `source_path` 聚合全部
/// 同身份键（planner 按文件构造、HIR 按 group 合并时同一声明身份可落在多个键上）。
fn family_by_binding_key(instances: &BTreeMap<DeclarationKey, CallableFamily>, key: &DeclarationKey) -> CallableFamily {
    if let Some(family) = instances.get(key) {
        return family.clone();
    }
    let mut union = CallableFamily::default();
    for (candidate, family) in instances {
        if candidate.dependency_key == key.dependency_key
            && candidate.namespace == key.namespace
            && candidate.kind == key.kind
            && candidate.owner == key.owner
            && candidate.name == key.name
        {
            union.declarations.extend(family.declarations.iter().copied());
            union.instances.extend(family.instances.iter().copied());
            union.signatures.extend(family.signatures.iter().map(|(instance, signature)| (*instance, signature.clone())));
        }
    }
    union
}

/// 实例签名是否仍含未代入的泛型 binder（开放签名不得承接闭合调用，也不参与配对）。
fn host_signature_is_open(signature: &(Vec<ValkyrieType>, ValkyrieType)) -> bool {
    signature.0.iter().chain(std::iter::once(&signature.1)).any(has_unresolved_generic_arguments)
}

/// HIR 已注册 callable 身份 → inventory 声明身份的映射（声明身份 → 实例族）。
///
/// 映射只消费 HIR 已注册的 `declaration` / `instance`；声明种类与拥有者必须与语义闭包
/// `collect_item` / `collect_members` / `collect_imply` 的登记一致，否则成员合同
/// 会被当成“未进入闭包”而静默跳过。名字键（`DeclarationKey`）只在作用域内解析，
/// 不能代替已解析身份：同一编码名下出现多个声明身份是合法重载集合（如
/// `Utf8Builder::append` 的 `char` / `&Utf8Text` 重载），扫描本身不得拒绝；
/// 歧义判定推迟到绑定查找——只有被 Resolver 绑定实际引用的键出现多声明身份、
/// 或同一声明身份冻结出多个实例（身份腐坏）才按结构化失败处理。
/// 单态化铸造副本（`instantiated_callables`）不参与名字扫描，
/// 按同一声明 `ItemId` 挂到实例族下——调用点只指向铸造实例，合同必须覆盖整个族。
fn host_callable_instances(hirs: &[&HirModule]) -> Result<BTreeMap<DeclarationKey, CallableFamily>, ParseError> {
    let mut collected = BTreeMap::<DeclarationKey, CallableFamily>::new();
    for module in hirs {
        for function in &module.functions {
            collect_callable_instance(
                &mut collected,
                declaration_key(module, DeclarationKind::Micro, &namespace_parts(&function.declaring_namespace), None, function.name.as_str()),
                function,
                None,
            );
        }
        for structure in &module.structs {
            let namespace: Vec<String> = structure.namespace.iter().map(|part| part.as_str().to_string()).collect();
            let owner = structure
                .constructor_declaration
                .map(|declaration| ValkyrieType::Nominal { declaration, arguments: Vec::new() });
            for method in &structure.methods {
                collect_callable_instance(
                    &mut collected,
                    declaration_key(module, DeclarationKind::Method, &namespace, Some(structure.name.as_str()), method.name.as_str()),
                    method,
                    owner.as_ref(),
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
                    None,
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
                    Some(&implementation.target),
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
                    None,
                );
            }
        }
    }
    let mut instances = BTreeMap::new();
    let mut mint_owner = BTreeMap::<ItemId, DeclarationKey>::new();
    for (key, family) in collected {
        // 同一声明身份冻结出多个实例是身份腐坏（一个 `ItemId` 只应有一个声明级实例），
        // 结构化拒绝；多个不同声明身份共享一个编码名是合法重载集合，原样保留。
        for (declaration, declaration_instances) in &family.by_declaration {
            if declaration.is_some() && declaration_instances.len() > 1 {
                return Err(ParseError::Invalid {
                    message: format!("callable 声明 `{}` 对应多个声明身份或实例", key.encoded()), span: None
                });
            }
        }
        for declaration in family.by_declaration.keys().flatten() {
            mint_owner.insert(*declaration, key.clone());
        }
        instances.insert(key, family);
    }
    for module in hirs {
        for callable in &module.instantiated_callables {
            let (Some(declaration), Some(instance)) = (callable.function.declaration, callable.function.instance)
            else {
                continue;
            };
            let Some(key) = mint_owner.get(&declaration)
            else {
                continue;
            };
            if let Some(family) = instances.get_mut(key) {
                family.instances.insert(instance);
                family.signatures.insert(instance, host_callable_signature(&callable.function, callable.owner_type.as_ref()));
            }
        }
    }
    Ok(instances)
}

/// 声明身份 → 实例族条目：按声明 `ItemId` 分桶记录声明侧实例（同一声明出现多个
/// 声明级实例即身份腐坏），并汇总整个键的声明身份集合与实例族（声明级实例加上
/// 单态化铸造副本，后续按 `ItemId` 汇入）。
#[derive(Default, Clone)]
struct CallableFamily {
    by_declaration: BTreeMap<Option<ItemId>, BTreeSet<ItemInstanceId>>,
    declarations: BTreeSet<ItemId>,
    instances: BTreeSet<ItemInstanceId>,
    /// 实例冻结签名（形参表 + 返回类型），provider 按具体签名配对合同实例。
    signatures: BTreeMap<ItemInstanceId, (Vec<ValkyrieType>, ValkyrieType)>,
}

fn collect_callable_instance(
    collected: &mut BTreeMap<DeclarationKey, CallableFamily>,
    key: DeclarationKey,
    function: &HirFunction,
    owner: Option<&ValkyrieType>,
) {
    let family = collected.entry(key).or_default();
    if let Some(declaration) = function.declaration {
        family.declarations.insert(declaration);
    }
    if let Some(instance) = function.instance {
        family.instances.insert(instance);
        family.by_declaration.entry(function.declaration).or_default().insert(instance);
        family.signatures.insert(instance, host_callable_signature(function, owner));
    }
}

/// 实例冻结签名（形参表 + 返回类型）。
///
/// 声明槽位的 `AutoType`/`SelfType` `self` 按 owner 类型归一化，与单态化侧
/// `declared_param_type` 同一事实源；否则 imply 成员合同（`length(self)`）与
/// provider 显式形参（`host_length(value: Utf8Text)`）永远配不上。
fn host_callable_signature(function: &HirFunction, owner: Option<&ValkyrieType>) -> (Vec<ValkyrieType>, ValkyrieType) {
    let params = function
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| match (&parameter.ty, index, owner) {
            (ValkyrieType::AutoType | ValkyrieType::r#SelfType, 0, Some(owner)) => owner.clone(),
            (ty, _, _) => ty.clone(),
        })
        .collect();
    (params, function.return_type.clone())
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
///
/// 返回实际生效的合同→provider 调用改写表；二次链接用它把依赖原件规范化到
/// 与消费模块副本相同的绑定后状态，才能逐字段核对同一冻结身份。
#[doc(hidden)]
pub fn apply_host_provider_bindings(
    module: &mut MirModule,
    dependency_mirs: &[MirModule],
    bindings: &[ResolvedHostProviderBinding],
) -> Result<BTreeMap<ItemInstanceId, ItemInstanceId>, ParseError> {
    let remaps = if bindings.is_empty() { BTreeMap::new() } else { remap_bound_host_contracts(module, dependency_mirs, bindings)? };
    enforce_resolved_host_contract_bindings(module)?;
    Ok(remaps)
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

/// 按 Resolver 绑定把合同调用逐对重映射到 provider，并安装 provider 定义/导入。
///
/// 合同身份覆盖整个实例族：闭合实例按签名配对重定向到 provider 闭合副本，
/// 族内全部合同体与导入（声明体与实例化副本）一并退役，不得留下进入后端的孤儿。
/// 返回实际生效的合同→provider 改写表。
fn remap_bound_host_contracts(
    module: &mut MirModule,
    dependency_mirs: &[MirModule],
    bindings: &[ResolvedHostProviderBinding],
) -> Result<BTreeMap<ItemInstanceId, ItemInstanceId>, ParseError> {
    let modules: Vec<&MirModule> = std::iter::once(&*module).chain(dependency_mirs.iter()).collect();
    let mut plans = Vec::new();

    for binding in bindings {
        if !binding.contract_instances.iter().any(|instance| callable_present(&modules, *instance)) {
            continue;
        }
        let mut provider_functions = Vec::new();
        for provider_instance in binding.provider_pairs.values() {
            if !callable_present(&modules, *provider_instance) {
                return Err(ParseError::Invalid {
                    message: format!("host provider `{}` 未在源码闭包中定义", provider_instance),
                    span: None,
                });
            }
            if let Some(function) =
                modules.iter().flat_map(|dependency| dependency.functions.iter()).find(|function| function.instance == Some(*provider_instance)).cloned()
            {
                provider_functions.push((*provider_instance, function));
            }
        }
        plans.push(PlannedBinding {
            contract_instances: binding.contract_instances.clone(),
            pairs: binding.provider_pairs.clone(),
            provider_functions,
        });
    }

    // 先完成全部合同的身份映射校验，再安装 provider 定义；冲突绑定不得留下部分副作用。
    let mut remaps = BTreeMap::<ItemInstanceId, ItemInstanceId>::new();
    for plan in &plans {
        for (contract_instance, provider_instance) in &plan.pairs {
            if contract_instance == provider_instance {
                continue;
            }
            if let Some(previous) = remaps.insert(*contract_instance, *provider_instance)
                && previous != *provider_instance
            {
                return Err(ParseError::Invalid { message: "host contract 不能同时绑定到多个 provider".into(), span: None });
            }
        }
    }

    // provider 的外部导入合同：无体 provider 由目标指令实现，其链接事实在依赖模块的
    // external_calls 里。重映射会移除原合同导入，必须把 provider 自己的导入合同
    // 一并迁入消费模块，否则调用既无定义也无导入。
    let selected_provider_instances = plans.iter().flat_map(|plan| plan.pairs.values().copied()).collect::<BTreeSet<_>>();
    let provider_imports = modules
        .iter()
        .flat_map(|dependency| dependency.external_calls.iter())
        .filter(|contract| contract.instance.is_some())
        .filter(|contract| selected_provider_instances.contains(&contract.instance.expect("provider import instance")))
        .map(|contract| (contract.instance.expect("provider import instance"), contract.clone()))
        .collect::<BTreeMap<ItemInstanceId, MirExternalCallContract>>();

    for plan in &plans {
        let defined = plan.provider_functions.iter().map(|(instance, _)| *instance).collect::<BTreeSet<_>>();
        for (provider_instance, provider_function) in &plan.provider_functions {
            install_provider_definition(module, *provider_instance, provider_function.clone())?;
        }
        for provider_instance in plan.pairs.values() {
            if defined.contains(provider_instance) {
                continue;
            }
            if let Some(contract) = provider_imports.get(provider_instance) {
                install_provider_import(module, contract)?;
            }
        }
    }

    if remaps.is_empty() {
        return Ok(remaps);
    }

    for function in &mut module.functions {
        remap_call_targets(function, &remaps);
    }

    // 退役整个绑定合同族（不只配对成功的实例）：开放声明级合同若被 BFS 拉入，
    // 残留导入或定义同样会成为后端孤儿。
    for plan in &plans {
        for contract_instance in &plan.contract_instances {
            module.callable_symbols.remove(contract_instance);
            module.external_calls.retain(|contract| contract.instance != Some(*contract_instance));
            // 合同体退役（含实例化副本）：调用已全部重映射到 provider 后，合同实现不再
            // 有任何调用身份；链接 BFS 在重映射之前已把合同体与铸造副本拉入 linked 模块，
            // 若不在此移除，它们将作为孤儿定义留在模块中并进入后端校验（其聚合字段指令
            // 与 provider 表示不一致即被拒绝）。
            module.functions.retain(|function| function.instance != Some(*contract_instance));
        }
    }

    Ok(remaps)
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
        module.register_callable_symbol(provider_instance, symbol).map_err(|e| ParseError::Invalid { message: e, span: None })?;
        return Ok(());
    }
    module.functions.push(provider_function.clone());
    let symbol = format!("{}#{}", provider_function.display_symbol, provider_instance.index());
    module.register_callable_symbol(provider_instance, symbol).map_err(|e| ParseError::Invalid { message: e, span: None })?;
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

/// 按合同→provider 改写表重写函数体内的 Call 目标。
///
/// 链接器在二次链接核对 installed 副本合同时复用同一改写事实，不得旁路复制逻辑。
pub(super) fn remap_call_targets(function: &mut MirFunction, remaps: &BTreeMap<ItemInstanceId, ItemInstanceId>) {
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

/// 为单态化固定点播种泛型 `[host_provider]` 的具体代入请求。
///
/// 返回「合同声明下标 → provider 声明下标」映射：合同调用点的具体代入同时
/// 决定 provider 实例签名，`instantiate_generic_callables` 据此在铸造合同
/// 闭合副本的同一固定点铸造 provider 的闭合副本；provider 体内的宿主导入
/// 调用随之闭合，合同与实现共用同一条实例化路径。只覆盖两侧均为单一声明
/// 身份的绑定，歧义留给 [`resolve_host_provider_bindings`] 结构化拒绝；
/// 非泛型 provider 无需播种（声明级实例本身闭合，配对直接命中）。
pub(crate) fn host_provider_instantiation_seeds(
    hirs: &[HirModule],
    bindings: &[CompilerHostProviderBinding],
) -> Result<BTreeMap<u32, u32>, ParseError> {
    if bindings.is_empty() {
        return Ok(BTreeMap::new());
    }
    let hirs = hirs.iter().collect::<Vec<_>>();
    let instances = host_callable_instances(&hirs)?;
    let mut seeds = BTreeMap::new();
    for binding in bindings {
        let contract_family = family_by_binding_key(&instances, &binding.contract);
        let provider_family = family_by_binding_key(&instances, &binding.provider);
        if contract_family.declarations.len() != 1 || provider_family.declarations.len() != 1 {
            continue;
        }
        let contract_declaration = *contract_family.declarations.iter().next().expect("single contract declaration");
        let provider_declaration = *provider_family.declarations.iter().next().expect("single provider declaration");
        seeds.insert(contract_declaration.index(), provider_declaration.index());
    }
    Ok(seeds)
}

/// 物化可达性门的 host provider 根：Resolver 选定 `[host_provider]` 绑定的
/// callable 声明身份集合。
///
/// 与 [`resolve_host_provider_bindings`] 共用同一族查找事实源（精确匹配 +
/// 忽略 `source_path` 的 fallback）。provider 体必须物化：合同重映射在链接
/// 边界要求 provider 实例存在于 MIR，否则报「host provider 未在源码闭包中定义」。
pub(crate) fn host_provider_root_declarations(
    hirs: &[&HirModule],
    bindings: &[CompilerHostProviderBinding],
) -> Result<BTreeSet<ItemId>, ParseError> {
    if bindings.is_empty() {
        return Ok(BTreeSet::new());
    }
    let instances = host_callable_instances(hirs)?;
    let mut roots = BTreeSet::new();
    for binding in bindings {
        let family = instances.get(&binding.provider).or_else(|| {
            instances
                .iter()
                .find(|(key, _)| {
                    key.dependency_key == binding.provider.dependency_key
                        && key.namespace == binding.provider.namespace
                        && key.kind == binding.provider.kind
                        && key.owner == binding.provider.owner
                        && key.name == binding.provider.name
                })
                .map(|(_, family)| family)
        });
        if let Some(family) = family {
            roots.extend(family.declarations.iter().copied());
        }
    }
    Ok(roots)
}
