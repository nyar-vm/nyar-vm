use std::{
    collections::{BTreeMap, BTreeSet},
    fs,

};

use crate::compiler::semantic::closure::DeclarationKey;
use crate::compiler::syntax::oak::parse_source;
use nyar::CanonicalTarget;

use super::capability::{provider_matches_target, provider_required_capabilities, target_capability_set};
use super::host_inventory_ast::collect_host_inventory_from_root;
use super::target_cfg::provider_cfg_matches_target;
use super::types::{PlannedHostContract, PlannedHostProvider, PlannedSemanticSourceGroup, PlannerError, PlannerResult};

#[derive(Debug, Default)]
pub(crate) struct HostInventory {
    pub(crate) contracts: Vec<PlannedHostContract>,
    pub(crate) providers: Vec<PlannedHostProvider>,
    pub(crate) selected_providers: Vec<PlannedHostProvider>,
}

pub(crate) fn collect_host_inventory(
    groups: &[PlannedSemanticSourceGroup],
    target: CanonicalTarget,
    demanded_declaration_keys: Option<&BTreeSet<DeclarationKey>>,
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
    inventory.selected_providers =
        select_host_providers(&inventory.contracts, &inventory.providers, demanded_declaration_keys, target)?;
    Ok(inventory)
}

fn select_host_providers(
    contracts: &[PlannedHostContract],
    providers: &[PlannedHostProvider],
    demanded_declaration_keys: Option<&BTreeSet<DeclarationKey>>,
    target: CanonicalTarget,
) -> PlannerResult<Vec<PlannedHostProvider>> {
    let available_capabilities = target_capability_set(target);
    let available_list: Vec<String> = available_capabilities.iter().cloned().collect();
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
    for contract in contracts {
        if let Some(demanded) = demanded_declaration_keys {
            if !demanded.contains(&contract.key) {
                continue;
            }
        }
        let Some(candidates) = providers_by_contract.get(&contract.id)
        else {
            if demanded_declaration_keys.map(|demanded| demanded.contains(&contract.key)).unwrap_or(false)
                && !contract.has_local_fallback_body
            {
                return Err(PlannerError::HostProviderCapabilityMissing {
                    contract: contract.id.clone(),
                    providers: Vec::new(),
                    available_capabilities: available_list.clone(),
                });
            }
            continue;
        };

        let compatible: Vec<&PlannedHostProvider> = candidates
            .iter()
            .copied()
            .filter(|provider| provider_cfg_matches_target(provider, target))
            .filter(|provider| provider_matches_target(provider, &available_capabilities, target))
            .collect();

        if compatible.is_empty() {
            return Err(PlannerError::HostProviderCapabilityMissing {
                contract: contract.id.clone(),
                providers: candidates
                    .iter()
                    .map(|item| {
                        let required = provider_required_capabilities(item, target);
                        format!("{} requires [{}] at {}:{}", item.symbol, required.join(", "), item.source_file.display(), item.line)
                    })
                    .collect(),
                available_capabilities: available_list.clone(),
            });
        }

        if compatible.len() > 1 {
            return Err(PlannerError::ConflictingHostProviders {
                contract: contract.id.clone(),
                providers: compatible
                    .iter()
                    .map(|item| format!("{} at {}:{}", item.symbol, item.source_file.display(), item.line))
                    .collect(),
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
    Ok(selected)
}

fn normalized_contract_reference(reference: &str) -> String {
    reference
        .split(['.', ':'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("::")
}
