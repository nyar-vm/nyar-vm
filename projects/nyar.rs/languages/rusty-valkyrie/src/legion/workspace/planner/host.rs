use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use crate::compiler::{semantic::closure::DeclarationKey, syntax::oak::parse_source};
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
            let root = parse_source(&source).map_err(|error| PlannerError::CompileFailed { message: error.to_string() })?;
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
    let (selected, gated_out) =
        select_host_providers(&inventory.contracts, &inventory.providers, demanded_declaration_keys, target, enabled_features)?;
    inventory.selected_providers = selected;
    inventory.gated_out_providers = gated_out;
    Ok(inventory)
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
