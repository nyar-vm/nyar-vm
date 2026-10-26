use std::{collections::BTreeMap, fs, path::Path};

use crate::compiler::semantic::closure::{DeclarationKey, DeclarationKind};

use super::types::{PlannedHostContract, PlannedHostProvider, PlannedSemanticSourceGroup, PlannerError, PlannerResult};

#[derive(Debug, Default)]
pub(crate) struct HostInventory {
    pub(crate) contracts: Vec<PlannedHostContract>,
    pub(crate) providers: Vec<PlannedHostProvider>,
    pub(crate) selected_providers: Vec<PlannedHostProvider>,
}

pub(crate) fn collect_host_inventory(groups: &[PlannedSemanticSourceGroup], publish: &[String]) -> PlannerResult<HostInventory> {
    let mut inventory = HostInventory::default();
    for group in groups {
        let manifest_dir = fs::canonicalize(&group.manifest_dir)?;
        for source_file in &group.source_files {
            let resolved = fs::canonicalize(source_file)?;
            // 逻辑路径必须与 `source_snapshot` 送给 Compiler 的路径一致：
            // 声明身份由（依赖组，manifest 相对路径，namespace，种类，拥有者，短名）共同构成。
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
            collect_host_attributes_from_file(&group.dependency_key, &source_path, source_file, &source, &mut inventory);
        }
    }

    inventory
        .contracts
        .sort_by(|left, right| left.id.cmp(&right.id).then(left.source_file.cmp(&right.source_file)).then(left.line.cmp(&right.line)));
    // 模板分支可能重复声明同一身份；声明身份不含行号，重复项必须收敛为同一事实。
    inventory.contracts.dedup_by(|left, right| left.key == right.key);
    inventory.providers.sort_by(|left, right| {
        left.contract
            .cmp(&right.contract)
            .then(left.symbol.cmp(&right.symbol))
            .then(left.source_file.cmp(&right.source_file))
            .then(left.line.cmp(&right.line))
    });
    inventory.providers.dedup_by(|left, right| left.key == right.key && left.contract == right.contract);
    inventory.selected_providers = select_host_providers(&inventory.contracts, &inventory.providers, publish)?;
    Ok(inventory)
}

fn collect_host_attributes_from_file(dependency_key: &str, source_path: &str, source_file: &Path, source: &str, inventory: &mut HostInventory) {
    let mut namespace: Vec<String> = Vec::new();
    let mut pending_host_contract: Option<usize> = None;
    let mut pending_host_providers: Vec<(String, usize)> = Vec::new();
    let mut owner_stack: Vec<(String, usize, DeclarationKind)> = Vec::new();
    let mut brace_depth: usize = 0;

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") || line.starts_with('⍝') {
            continue;
        }

        let open_brace_count = line.chars().filter(|ch| *ch == '{').count();
        let close_brace_count = line.chars().filter(|ch| *ch == '}').count();
        let parsed_owner = parse_owner_declaration(line);

        if let Some(parsed_namespace) = parse_namespace(line) {
            namespace = namespace_segments(parsed_namespace);
            continue;
        }

        if let Some(attributes) = parse_attribute_items(line) {
            for attribute in attributes {
                if attribute == "host_contract" {
                    pending_host_contract = Some(line_number);
                    continue;
                }

                if let Some(contract) = parse_host_provider_attribute(attribute) {
                    pending_host_providers.push((contract.to_string(), line_number));
                }
            }
            continue;
        }

        let Some(symbol_name) = parse_symbol_name(line)
        else {
            let next_brace_depth = brace_depth.saturating_add(open_brace_count).saturating_sub(close_brace_count);
            if let Some((owner_name, owner_kind)) = parsed_owner {
                if next_brace_depth > brace_depth {
                    owner_stack.push((owner_name, next_brace_depth, owner_kind));
                }
            }
            brace_depth = next_brace_depth;
            while owner_stack.last().map(|(_, depth, _)| brace_depth < *depth).unwrap_or(false) {
                owner_stack.pop();
            }
            continue;
        };
        let (owner, kind) = match owner_stack.last() {
            Some((owner_name, _, owner_kind)) => (Some(owner_name.as_str()), *owner_kind),
            None => (None, DeclarationKind::Micro),
        };
        let qualified_symbol = qualified_declaration_name(&namespace, owner, symbol_name);
        let key = DeclarationKey {
            dependency_key: dependency_key.to_string(),
            source_path: source_path.to_string(),
            namespace: namespace.clone(),
            kind,
            owner: owner.map(str::to_string),
            name: symbol_name.to_string(),
        };

        if let Some(contract_line) = pending_host_contract.take() {
            inventory.contracts.push(PlannedHostContract {
                id: qualified_symbol.clone(),
                key: key.clone(),
                source_file: source_file.to_path_buf(),
                line: contract_line,
            });
        }

        for (contract, provider_line) in pending_host_providers.drain(..) {
            inventory.providers.push(PlannedHostProvider {
                contract,
                contract_key: None,
                symbol: qualified_symbol.clone(),
                key: key.clone(),
                source_file: source_file.to_path_buf(),
                line: provider_line,
            });
        }

        let next_brace_depth = brace_depth.saturating_add(open_brace_count).saturating_sub(close_brace_count);
        if let Some((owner_name, owner_kind)) = parsed_owner {
            if next_brace_depth > brace_depth {
                owner_stack.push((owner_name, next_brace_depth, owner_kind));
            }
        }
        brace_depth = next_brace_depth;
        while owner_stack.last().map(|(_, depth, _)| brace_depth < *depth).unwrap_or(false) {
            owner_stack.pop();
        }
    }
}

fn select_host_providers(
    contracts: &[PlannedHostContract],
    providers: &[PlannedHostProvider],
    publish: &[String],
) -> PlannerResult<Vec<PlannedHostProvider>> {
    // 只保留声明了对应 contract 的 provider；contract 本身由 language inventory 与 closure 决定，
    // 不由 target 侧裁剪。属性里允许 `.` / `::` 两种书写，匹配前统一为 `::` 分隔的限定名。
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
        let Some(candidates) = providers_by_contract.get(&contract.id)
        else {
            continue;
        };

        let chosen = if candidates.len() == 1 {
            candidates[0]
        }
        else {
            candidates.iter().copied().min_by_key(|provider| host_provider_priority(&provider.source_file, publish)).ok_or_else(|| {
                PlannerError::ConflictingHostProviders {
                    contract: contract.id.clone(),
                    providers: candidates
                        .iter()
                        .map(|item| format!("{} at {}:{}", item.symbol, item.source_file.display(), item.line))
                        .collect(),
                }
            })?
        };

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

fn host_provider_priority(source_file: &Path, publish: &[String]) -> u8 {
    let path = source_file.to_string_lossy();
    if publish.iter().any(|item| item == "unity-player") && path.contains("unity.engine.sdk") {
        return 0;
    }
    if publish.iter().any(|item| item == "mini-game") && path.contains("tencent.wechat.sdk") {
        return 0;
    }
    if publish.iter().any(|item| item == "mini-program") && path.contains("tencent.wechat.miniprogram.sdk") {
        return 0;
    }
    if path.contains("std.adaptor.") {
        return 2;
    }
    1
}

fn parse_namespace(line: &str) -> Option<&str> {
    line.strip_prefix("namespace ").or_else(|| line.strip_prefix("namespace! ")).and_then(|value| value.strip_suffix(';')).map(str::trim)
}

fn parse_attribute_items(line: &str) -> Option<Vec<&str>> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?.trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }

    let mut items = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0usize;
    let mut in_string = false;
    let mut previous_was_escape = false;

    for (index, ch) in inner.char_indices() {
        match ch {
            '"' if !previous_was_escape => {
                in_string = !in_string;
            }
            '(' if !in_string => {
                paren_depth += 1;
            }
            ')' if !in_string => {
                paren_depth = paren_depth.saturating_sub(1);
            }
            ',' if !in_string && paren_depth == 0 => {
                items.push(inner[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }

        if ch == '\\' {
            previous_was_escape = !previous_was_escape;
        }
        else {
            previous_was_escape = false;
        }
    }

    items.push(inner[start..].trim());
    Some(items)
}

fn parse_host_provider_attribute(attribute: &str) -> Option<&str> {
    let inner = attribute.strip_prefix("host_provider(")?.strip_suffix(')')?.trim();
    if let Some(value) = inner.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        return Some(value);
    }

    Some(inner)
}

fn parse_symbol_name(line: &str) -> Option<&str> {
    let micro_offset = line.find("micro ")?;
    let rest = &line[micro_offset + "micro ".len()..];
    let end = rest.find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')).unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    Some(&rest[..end])
}

/// 解析成员拥有者及其声明种类：`imply` 块产生 `ImplyMethod`，`class`/`structure`/`trait`
/// 成员产生 `Method`。种类是冻结声明身份的一部分，猜错会导致 Compiler 查不到声明。
fn parse_owner_declaration(line: &str) -> Option<(String, DeclarationKind)> {
    let (rest, kind) = if let Some(rest) = line.strip_prefix("imply ") {
        (rest, DeclarationKind::ImplyMethod)
    }
    else if let Some(rest) = line.strip_prefix("class ").or_else(|| line.strip_prefix("structure ")).or_else(|| line.strip_prefix("trait ")) {
        (rest, DeclarationKind::Method)
    }
    else {
        return None;
    };
    let end = rest.find(|ch: char| ch.is_whitespace() || matches!(ch, '{' | '<' | '>' | '(' | ')' | ';' | '=')).unwrap_or(rest.len());
    let owner = rest[..end].rsplit([':', '.']).find(|segment| !segment.is_empty())?;
    Some((owner.to_string(), kind))
}

/// namespace 段：源码允许 `.` / `::` 两种书写，统一拆成段。
fn namespace_segments(namespace: &str) -> Vec<String> {
    namespace.split(['.', ':']).map(str::trim).filter(|segment| !segment.is_empty()).map(str::to_string).collect()
}

fn qualified_declaration_name(namespace: &[String], owner: Option<&str>, symbol_name: &str) -> String {
    namespace.iter().map(String::as_str).chain(owner).chain(std::iter::once(symbol_name)).collect::<Vec<_>>().join("::")
}

/// 属性里的契约引用统一为 `::` 分隔的限定名，只用于匹配，不据此反推声明身份。
fn normalized_contract_reference(reference: &str) -> String {
    namespace_segments(reference).join("::")
}
