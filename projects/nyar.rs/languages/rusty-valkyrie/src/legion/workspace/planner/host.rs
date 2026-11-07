use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use crate::compiler::{
    semantic::closure::{DeclarationIndexEntry, DeclarationIndexes, DeclarationKey, ReferencePath, index_parsed_root},
    syntax::oak::parse_source_with_identity,
};
use nyar::CanonicalTarget;

use super::{
    capability::{provider_matches_target, target_host_capability_space},
    condition_eval::{provider_passes_gate_axis, provider_passes_target_axis},
    host_inventory_ast::collect_host_inventory_from_root,
    types::{PlannedHostContract, PlannedHostProvider, PlannedHostProviderExclusion, PlannedSemanticSourceGroup, PlannerError, PlannerResult},
};

#[derive(Debug, Default)]
pub(crate) struct HostInventory {
    pub(crate) contracts: Vec<PlannedHostContract>,
    pub(crate) providers: Vec<PlannedHostProvider>,
    pub(crate) selected_providers: Vec<PlannedHostProvider>,
    /// 因 feature gate 未启用而被排除的 provider（编译期诊断事实）。
    pub(crate) gated_out_providers: Vec<PlannedHostProviderExclusion>,
    /// 声明自身 foreign binding 给出的能力（仅非空项），按冻结声明身份索引。
    pub(crate) declaration_capabilities: BTreeMap<DeclarationKey, BTreeSet<String>>,
    /// 声明体内真实调用点的 callee 路径；与语义闭包共用同一份 AST 引用收集。
    pub(crate) declaration_calls: BTreeMap<DeclarationKey, BTreeSet<ReferencePath>>,
    /// 本次 inventory 的声明索引原稿：调用边按与语义闭包相同的可见性合同解析。
    pub(crate) declaration_entries: Vec<DeclarationIndexEntry>,
}

pub(crate) fn collect_host_inventory(
    groups: &[PlannedSemanticSourceGroup],
    target: CanonicalTarget,
    demanded_declaration_keys: Option<&BTreeSet<DeclarationKey>>,
    enabled_features: Option<&std::collections::BTreeSet<String>>,
) -> PlannerResult<HostInventory> {
    let mut inventory = HostInventory::default();
    for group in groups {
        let manifest_dir = fs::canonicalize(&group.manifest_dir)?;
        for source_file in &group.source_files {
            let resolved = fs::canonicalize(source_file)?;
            let source_path = resolved
                .strip_prefix(&manifest_dir)
                .map_err(|_| {
                    PlannerError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("源码文件 {} 不属于源码组 `{}` 的 manifest 目录", source_file.display(), group.dependency_key),
                    ))
                })?
                .to_string_lossy()
                .replace('\\', "/");
            let source = fs::read_to_string(source_file)?;
            let root = parse_source_with_identity(&source, &source_path)
                .map_err(|error| PlannerError::CompileFailed { message: error.to_string() })?;
            // 与 host inventory 共用同一棵解析树建声明索引：能力闭包的调用边解析
            // 与语义闭包的名字事实一致，不另造第二份名字表。
            let indexed = index_parsed_root(&group.dependency_key, &source_path, &source, &root);
            inventory.declaration_entries.extend(indexed.declarations);
            collect_host_inventory_from_root(&group.dependency_key, &source_path, source_file, &source, &root, &mut inventory);
        }
    }

    inventory
        .contracts
        .sort_by(|left, right| left.id.cmp(&right.id).then(left.source_file.cmp(&right.source_file)).then(left.line.cmp(&right.line)));
    inventory.contracts.dedup_by(|left, right| left.key == right.key);
    inventory.providers.sort_by(|left, right| {
        left.contract
            .cmp(&right.contract)
            .then(left.symbol.cmp(&right.symbol))
            .then(left.source_file.cmp(&right.source_file))
            .then(left.line.cmp(&right.line))
    });
    inventory.providers.dedup_by(|left, right| left.key == right.key && left.contract == right.contract);
    apply_capability_closure(&mut inventory, groups);
    let (selected, gated_out) =
        select_host_providers(&inventory.contracts, &inventory.providers, demanded_declaration_keys, target, enabled_features)?;
    inventory.selected_providers = selected;
    inventory.gated_out_providers = gated_out;
    Ok(inventory)
}

/// 把 provider 的 required capabilities 扩展为「声明属性 ∪ 体内可达调用边闭包」。
///
/// provider 常把宿主能力藏在同文件的私有 glue 里（`host_provider` 本体只做转接，
/// `[js_builtin]` 在它调用的 glue 声明上）。能力归属必须沿真实 callee 传播：
///
/// - 边来自 [`References::calls`]，即 AST 遍历中 `ApplyCall` 的 callee，不是「文件里
///   提到了某个名字」；未被调用的同文件 glue 不会贡献能力。
/// - 解析复用 [`DeclarationIndexes`] 与 `direct_dependencies` 可见性，和语义闭包的
///   `source_group_closure_with_host_bindings` 是同一套名字事实。
/// - 这是编译期 host binding 的前置上界：`host_callable_instances` 与
///   `resolve_host_provider_bindings` 在 HIR 物化后按实例身份重新判定「谁被真调用」，
///   本层只负责让 provider 的候选能力与它体内的调用边一致。
fn apply_capability_closure(inventory: &mut HostInventory, groups: &[PlannedSemanticSourceGroup]) {
    let inventory_keys = groups.iter().map(|group| group.dependency_key.as_str()).collect::<BTreeSet<_>>();
    let visibility = groups
        .iter()
        .map(|group| {
            let mut visible = BTreeSet::from([group.dependency_key.clone()]);
            visible.extend(group.direct_dependencies.iter().filter(|dependency| inventory_keys.contains(dependency.as_str())).cloned());
            (group.dependency_key.clone(), visible)
        })
        .collect::<BTreeMap<String, BTreeSet<String>>>();
    let mut indexes = DeclarationIndexes::default();
    for entry in std::mem::take(&mut inventory.declaration_entries) {
        indexes.register(entry);
    }
    indexes.link(&visibility);

    let resolved = inventory
        .providers
        .iter()
        .map(|provider| {
            reachable_capabilities(&provider.key, &inventory.declaration_capabilities, &inventory.declaration_calls, &indexes, &visibility)
        })
        .collect::<Vec<_>>();
    for (provider, capabilities) in inventory.providers.iter_mut().zip(resolved) {
        provider.required_capabilities = capabilities.into_iter().collect();
    }
}

/// 从声明本体出发，沿真实 callee 边收集可达声明的能力标签。
fn reachable_capabilities(
    seed: &DeclarationKey,
    declaration_capabilities: &BTreeMap<DeclarationKey, BTreeSet<String>>,
    declaration_calls: &BTreeMap<DeclarationKey, BTreeSet<ReferencePath>>,
    indexes: &DeclarationIndexes,
    visibility: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let empty_visible = BTreeSet::new();
    let empty_calls = BTreeSet::new();
    let mut capabilities = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut frontier = vec![seed.clone()];
    while let Some(key) = frontier.pop() {
        if !visited.insert(key.clone()) {
            continue;
        }
        if let Some(own) = declaration_capabilities.get(&key) {
            capabilities.extend(own.iter().cloned());
        }
        let visible = visibility.get(&key.dependency_key).unwrap_or(&empty_visible);
        for reference in declaration_calls.get(&key).unwrap_or(&empty_calls) {
            frontier.extend(indexes.resolve(reference, &key.namespace, visible));
        }
    }
    capabilities
}

fn select_host_providers(
    contracts: &[PlannedHostContract],
    providers: &[PlannedHostProvider],
    demanded_declaration_keys: Option<&BTreeSet<DeclarationKey>>,
    target: CanonicalTarget,
    enabled_features: Option<&std::collections::BTreeSet<String>>,
) -> PlannerResult<(Vec<PlannedHostProvider>, Vec<PlannedHostProviderExclusion>)> {
    let available_capabilities = target_host_capability_space(target);
    let contracts_by_id: BTreeMap<&str, &PlannedHostContract> = contracts.iter().map(|item| (item.id.as_str(), item)).collect();
    let mut providers_by_contract: BTreeMap<String, Vec<&PlannedHostProvider>> = BTreeMap::new();
    for provider in providers {
        let contract_id = normalized_contract_reference(&provider.contract);
        if !contracts_by_id.contains_key(contract_id.as_str()) {
            continue;
        }
        providers_by_contract.entry(contract_id).or_default().push(provider);
    }

    let mut selected = Vec::new();
    let mut gated_out = Vec::new();
    for contract in contracts {
        if let Some(demanded) = demanded_declaration_keys {
            if !demanded.contains(&contract.key) {
                continue;
            }
        }
        // 需求判定属于编译边界的「已解析 callee」事实，不是闭包保守超集：
        // 无候选 provider 的合同在这里只是「不选中」，缺实现的失败由
        // Compiler host binding 阶段按真实调用结构化上报。
        let Some(candidates) = providers_by_contract.get(&contract.id)
        else {
            continue;
        };

        let compatible: Vec<&PlannedHostProvider> = candidates
            .iter()
            .copied()
            .filter(|provider| provider_passes_target_axis(provider, target))
            .filter(|provider| provider_passes_gate_axis(provider, enabled_features))
            .filter(|provider| provider_matches_target(provider, &available_capabilities, target))
            .collect();

        // 候选存在但都不兼容当前 target 时同样只是「不选中」：
        // 选择语义（target axis / gate axis / capability 匹配）保持不变，
        // 合同是否真的被调用由编译边界裁决。
        if compatible.is_empty() {
            // 记录被 feature gate 排除的候选：编译期 unbound-contract 诊断据此
            // 指出「provider 存在但被哪个 feature 排除」，不是凭空缺实现。
            for candidate in candidates {
                if !provider_passes_gate_axis(candidate, enabled_features) {
                    for gate in &candidate.gate_attributes {
                        if gate.key == "feature" {
                            gated_out.push(PlannedHostProviderExclusion { provider: (*candidate).clone(), feature: gate.value.clone() });
                        }
                    }
                }
            }
            continue;
        }

        if compatible.len() > 1 {
            return Err(PlannerError::ConflictingHostProviders {
                contract: contract.id.clone(),
                providers: compatible.iter().map(|item| format!("{} at {}:{}", item.symbol, item.source_file.display(), item.line)).collect(),
            });
        }

        let chosen = compatible[0];

        let mut chosen = chosen.clone();
        chosen.contract_key = Some(contract.key.clone());
        selected.push(chosen);
    }

    selected.sort_by(|left, right| {
        left.contract
            .cmp(&right.contract)
            .then(left.symbol.cmp(&right.symbol))
            .then(left.source_file.cmp(&right.source_file))
            .then(left.line.cmp(&right.line))
    });
    Ok((selected, gated_out))
}

pub(crate) fn normalized_contract_reference(reference: &str) -> String {
    reference.split(['.', ':']).map(str::trim).filter(|segment| !segment.is_empty()).collect::<Vec<_>>().join("::")
}
